use std::{fs::File, path::Path, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use symphonia::core::{
    audio::{AudioBufferRef, SampleBuffer},
    codecs::{CODEC_TYPE_NULL, CODEC_TYPE_OPUS},
    errors::Error,
    formats::FormatOptions,
    io::MediaSourceStream,
    meta::{MetadataOptions, StandardTagKey},
    probe::Hint,
};

use crate::playback_state::{PlaybackStateSender, TrackMetadata, VISUALIZER_WINDOW_SAMPLES};

pub struct DecodedTrack {
    pub metadata: TrackMetadata,
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

struct NativeOpusDecoder {
    decoder: *mut libopus_sys::OpusDecoder,
    channels: usize,
}

impl NativeOpusDecoder {
    fn new(sample_rate: u32, channels: usize) -> Result<Self> {
        let mut err = 0;
        let decoder = unsafe {
            libopus_sys::opus_decoder_create(sample_rate as i32, channels as i32, &mut err)
        };
        if err != libopus_sys::OPUS_OK as i32 || decoder.is_null() {
            anyhow::bail!("failed to create libopus decoder: error code {err}");
        }
        Ok(Self { decoder, channels })
    }

    fn decode_float(&mut self, data: &[u8], pcm: &mut [f32]) -> Result<usize> {
        let max_samples_per_channel = (pcm.len() / self.channels) as i32;
        let res = unsafe {
            libopus_sys::opus_decode_float(
                self.decoder,
                data.as_ptr(),
                data.len() as i32,
                pcm.as_mut_ptr(),
                max_samples_per_channel,
                0,
            )
        };
        if res < 0 {
            anyhow::bail!("opus_decode_float error code {res}");
        }
        Ok(res as usize)
    }
}

impl Drop for NativeOpusDecoder {
    fn drop(&mut self) {
        if !self.decoder.is_null() {
            unsafe {
                libopus_sys::opus_decoder_destroy(self.decoder);
            }
        }
    }
}

unsafe impl Send for NativeOpusDecoder {}

/// Decodes MP3, FLAC, and WAV into interleaved 32-bit float samples.
pub fn decode_file(path: impl AsRef<Path>) -> Result<Vec<f32>> {
    Ok(decode_track(path)?.samples)
}

/// Parses file metadata and decodes its samples before the track is eligible for playback.
pub fn decode_track(path: impl AsRef<Path>) -> Result<DecodedTrack> {
    let path = path.as_ref();
    let source = File::open(path).with_context(|| format!("unable to open {}", path.display()))?;
    let stream = MediaSourceStream::new(Box::new(source), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|extension| extension.to_str()) {
        hint.with_extension(extension);
    }
    let mut probed = symphonia::default::get_probe().format(
        &hint,
        stream,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;

    let default_title = path.file_stem().and_then(|name| name.to_str()).unwrap_or("Unknown title").to_owned();
    let mut metadata = TrackMetadata {
        title: default_title.clone(),
        artist: "Unknown Artist".to_owned(),
        album: "Unknown Album".to_owned(),
        duration: None,
    };

    let mut extract_meta = |tags: &[symphonia::core::meta::Tag]| {
        for tag in tags {
            let val = match &tag.value {
                symphonia::core::meta::Value::String(s) => s.trim().to_string(),
                val => val.to_string().trim().to_string(),
            };
            if val.is_empty() {
                continue;
            }
            match tag.std_key {
                Some(StandardTagKey::TrackTitle) => metadata.title = val,
                Some(StandardTagKey::Artist) => metadata.artist = val,
                Some(StandardTagKey::Album) => metadata.album = val,
                _ => {
                    let k = tag.key.to_ascii_uppercase();
                    if (k == "TITLE" || k == "TIT2") && (metadata.title.is_empty() || metadata.title == default_title) {
                        metadata.title = val;
                    } else if (k == "ARTIST" || k == "TPE1") && (metadata.artist.is_empty() || metadata.artist == "Unknown Artist") {
                        metadata.artist = val;
                    } else if (k == "ALBUM" || k == "TALB") && (metadata.album.is_empty() || metadata.album == "Unknown Album") {
                        metadata.album = val;
                    }
                }
            }
        }
    };

    if let Some(rev) = probed.metadata.get().as_ref().and_then(|m| m.current()) {
        extract_meta(rev.tags());
    }

    let mut format = probed.format;
    if let Some(rev) = format.metadata().current() {
        extract_meta(rev.tags());
    }

    let (track_id, track_codec_params, mut sample_rate, mut channels, duration) = {
        let track = format.default_track().context("file has no default audio track")?;
        if track.codec_params.codec == CODEC_TYPE_NULL {
            anyhow::bail!("file uses an unsupported audio codec");
        }
        let track_id = track.id;
        let sample_rate = track.codec_params.sample_rate.unwrap_or(0);
        let channels = track.codec_params.channels.map(|c| c.count() as u16).unwrap_or(0);
        let duration = if let (Some(time_base), Some(frames)) = (track.codec_params.time_base, track.codec_params.n_frames) {
            let dur = time_base.calc_time(frames);
            let secs = dur.seconds as f64 + f64::from(dur.frac);
            if secs > 0.0 {
                Some(Duration::from_secs_f64(secs))
            } else {
                None
            }
        } else {
            None
        };
        (track_id, track.codec_params.clone(), sample_rate, channels, duration)
    };
    if duration.is_some() {
        metadata.duration = duration;
    }
    let is_opus = track_codec_params.codec == CODEC_TYPE_OPUS;
    let mut samples = Vec::new();

    if is_opus {
        let opus_rate = match sample_rate {
            8000 | 12000 | 16000 | 24000 | 48000 => sample_rate,
            _ => 48000,
        };
        let opus_channels = if channels == 1 { 1 } else { 2 };
        let mut opus_decoder = NativeOpusDecoder::new(opus_rate, opus_channels)?;

        sample_rate = opus_rate;
        channels = opus_channels as u16;

        let mut pcm_buf = vec![0.0f32; 5760 * opus_channels];

        loop {
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(Error::IoError(error)) if error.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(error) => return Err(error.into()),
            };
            if packet.track_id() != track_id {
                continue;
            }
            if let Ok(decoded_frames) = opus_decoder.decode_float(&packet.data, &mut pcm_buf) {
                let count = decoded_frames * opus_channels;
                samples.extend_from_slice(&pcm_buf[..count]);
            }
        }
    } else {
        let mut decoder = symphonia::default::get_codecs().make(&track_codec_params, &Default::default())?;

        loop {
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(Error::IoError(error)) if error.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(error) => return Err(error.into()),
            };
            if packet.track_id() != track_id {
                continue;
            }
            match decoder.decode(&packet) {
                Ok(decoded) => {
                    if channels == 0 {
                        channels = decoded.spec().channels.count() as u16;
                    }
                    if sample_rate == 0 {
                        sample_rate = decoded.spec().rate;
                    }
                    append_f32(&mut samples, decoded);
                }
                Err(Error::DecodeError(_)) => continue,
                Err(error) => return Err(error.into()),
            }
        }

        if channels == 0 {
            channels = decoder.codec_params().channels.map(|c| c.count() as u16).unwrap_or(2);
        }
        if sample_rate == 0 {
            sample_rate = decoder.codec_params().sample_rate.unwrap_or(44100);
        }
    }

    if metadata.duration.is_none() && sample_rate > 0 && channels > 0 && !samples.is_empty() {
        let total_frames = samples.len() / usize::from(channels);
        metadata.duration = Some(Duration::from_secs_f64(total_frames as f64 / f64::from(sample_rate)));
    }

    Ok(DecodedTrack { metadata, samples, sample_rate, channels })
}

/// Publishes fixed-size PCM windows while decoding on a non-real-time worker.
pub fn publish_visualizer_windows(samples: &[f32], sender: &PlaybackStateSender) {
    for window in samples.chunks(VISUALIZER_WINDOW_SAMPLES) {
        sender.publish_visualizer_pcm(Arc::from(window));
    }
}

fn append_f32(samples: &mut Vec<f32>, decoded: AudioBufferRef<'_>) {
    let mut buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
    buffer.copy_interleaved_ref(decoded);
    samples.extend_from_slice(buffer.samples());
}

/// Adapts decoded audio to the target output device's channel count and sample rate.
pub fn resample_and_remap_channels(
    samples: &[f32],
    src_rate: u32,
    src_channels: u16,
    dst_rate: u32,
    dst_channels: u16,
) -> Vec<f32> {
    if samples.is_empty() || src_channels == 0 || dst_channels == 0 {
        return Vec::new();
    }

    // Step 1: Channel mapping
    let remaped = if src_channels == dst_channels {
        samples.to_vec()
    } else if src_channels == 1 && dst_channels == 2 {
        // Mono to Stereo: Duplicate sample to left and right channels
        let mut out = Vec::with_capacity(samples.len() * 2);
        for &s in samples {
            out.push(s);
            out.push(s);
        }
        out
    } else if src_channels == 2 && dst_channels == 1 {
        // Stereo to Mono: Average left and right
        let mut out = Vec::with_capacity(samples.len() / 2);
        for chunk in samples.chunks(2) {
            let s = if chunk.len() == 2 { (chunk[0] + chunk[1]) * 0.5 } else { chunk[0] };
            out.push(s);
        }
        out
    } else if src_channels > dst_channels {
        let ch = src_channels as usize;
        let mut out = Vec::with_capacity((samples.len() / ch) * dst_channels as usize);
        for frame in samples.chunks(ch) {
            let avg: f32 = frame.iter().sum::<f32>() / frame.len() as f32;
            for _ in 0..dst_channels {
                out.push(avg);
            }
        }
        out
    } else {
        let ch = src_channels as usize;
        let mut out = Vec::with_capacity((samples.len() / ch) * dst_channels as usize);
        for frame in samples.chunks(ch) {
            for i in 0..dst_channels as usize {
                out.push(frame[i % frame.len()]);
            }
        }
        out
    };

    // Step 2: Sample rate resampling
    if src_rate == dst_rate || src_rate == 0 || dst_rate == 0 {
        return remaped;
    }

    let ch = dst_channels as usize;
    let total_src_frames = remaped.len() / ch;
    if total_src_frames == 0 {
        return remaped;
    }

    let ratio = dst_rate as f64 / src_rate as f64;
    let total_dst_frames = (total_src_frames as f64 * ratio).round() as usize;
    let mut resampled = Vec::with_capacity(total_dst_frames * ch);

    for dst_idx in 0..total_dst_frames {
        let src_pos = dst_idx as f64 / ratio;
        let src_idx = src_pos.floor() as usize;
        let frac = (src_pos - src_idx as f64) as f32;

        let next_idx = (src_idx + 1).min(total_src_frames - 1);
        for c in 0..ch {
            let s0 = remaped[src_idx * ch + c];
            let s1 = remaped[next_idx * ch + c];
            let interpolated = s0 + frac * (s1 - s0);
            resampled.push(interpolated);
        }
    }

    resampled
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_mapping_mono_to_stereo() {
        let mono = vec![0.5, -0.5, 0.2];
        let stereo = resample_and_remap_channels(&mono, 44100, 1, 44100, 2);
        assert_eq!(stereo, vec![0.5, 0.5, -0.5, -0.5, 0.2, 0.2]);
    }

    #[test]
    fn test_channel_mapping_stereo_to_mono() {
        let stereo = vec![0.4, 0.6, -0.2, -0.4];
        let mono = resample_and_remap_channels(&stereo, 44100, 2, 44100, 1);
        assert_eq!(mono, vec![0.5, -0.3]);
    }

    #[test]
    fn test_sample_rate_resampling() {
        let src = vec![0.0, 1.0, 0.0];
        let resampled = resample_and_remap_channels(&src, 1000, 1, 2000, 1);
        assert_eq!(resampled.len(), 6);
        assert!((resampled[0] - 0.0).abs() < 1e-5);
    }

    #[test]
    fn test_decode_webm_format() {
        let temp_webm = std::env::temp_dir().join("kanono_test_track.webm");
        let status = std::process::Command::new("ffmpeg")
            .args([
                "-f", "lavfi",
                "-i", "sine=frequency=440:duration=1",
                "-c:a", "libvorbis",
                "-y",
                temp_webm.to_str().unwrap(),
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();

        if let Ok(s) = status {
            if s.success() {
                let decoded = decode_track(&temp_webm).expect("WebM file should decode successfully");
                assert!(decoded.sample_rate > 0);
                assert!(decoded.channels > 0);
                assert!(!decoded.samples.is_empty());
                assert!(decoded.metadata.duration.is_some());
                let _ = std::fs::remove_file(&temp_webm);
            }
        }
    }

    #[test]
    fn test_decode_webm_opus() {
        let temp_webm = std::env::temp_dir().join("kanono_test_track_opus.webm");
        let status = std::process::Command::new("ffmpeg")
            .args([
                "-f", "lavfi",
                "-i", "sine=frequency=440:duration=1",
                "-c:a", "libopus",
                "-y",
                temp_webm.to_str().unwrap(),
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();

        if let Ok(s) = status {
            if s.success() {
                let decoded = decode_track(&temp_webm).expect("Opus WebM file should decode successfully");
                assert_eq!(decoded.sample_rate, 48000);
                assert!(decoded.channels > 0);
                assert!(!decoded.samples.is_empty());
                assert!(decoded.metadata.duration.is_some());
                let _ = std::fs::remove_file(&temp_webm);
            }
        }

        // Test user's actual files if present on disk
        let user_dir = std::path::Path::new("/home/tankisocorp/Music/Sebata masene");
        if user_dir.exists() {
            if let Some(entry) = std::fs::read_dir(user_dir).unwrap().flatten().find(|e| e.path().extension().is_some_and(|ext| ext == "webm")) {
                let path = entry.path();
                let decoded = decode_track(&path).unwrap_or_else(|e| panic!("Failed to decode {}: {e}", path.display()));
                assert_eq!(decoded.sample_rate, 48000);
                assert_eq!(decoded.channels, 2);
                assert!(!decoded.samples.is_empty());
                let rg = crate::replaygain::analyze_and_tag(&path).unwrap_or_else(|e| panic!("ReplayGain failed on {}: {e}", path.display()));
                assert!(rg.integrated_lufs.is_finite());
            }
        }
    }
}