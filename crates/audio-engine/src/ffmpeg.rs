use std::{
    io::Read,
    path::Path,
    process::{Command, Stdio},
    time::Duration,
};

use anyhow::{Context, Result};

use crate::decoder::DecodedTrack;
use crate::playback_state::TrackMetadata;

/// Check if ffmpeg is installed and callable on the host machine.
pub fn is_ffmpeg_available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Metadata extracted from a media file via ffprobe or ffmpeg.
#[derive(Debug, Default, Clone)]
pub struct MediaMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub year: Option<i32>,
    pub track_number: Option<i32>,
    pub duration: Option<Duration>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
}

/// Probes metadata and duration using ffprobe.
pub fn probe_file(path: &Path) -> Option<MediaMetadata> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration:format_tags=title,artist,album,genre,date,track:stream=sample_rate,channels:stream_tags=title,artist,album,genre,date,track",
            "-of",
            "default=noprint_wrappers=1",
        ])
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let out_str = String::from_utf8_lossy(&output.stdout);
    let mut meta = MediaMetadata::default();

    for line in out_str.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("duration=") {
            if let Ok(secs) = rest.parse::<f64>() {
                if secs > 0.0 {
                    meta.duration = Some(Duration::from_secs_f64(secs));
                }
            }
        } else if let Some(rest) = line.strip_prefix("sample_rate=") {
            if let Ok(rate) = rest.parse::<u32>() {
                if rate > 0 && meta.sample_rate.is_none() {
                    meta.sample_rate = Some(rate);
                }
            }
        } else if let Some(rest) = line.strip_prefix("channels=") {
            if let Ok(ch) = rest.parse::<u16>() {
                if ch > 0 && meta.channels.is_none() {
                    meta.channels = Some(ch);
                }
            }
        } else if let Some(rest) = line.strip_prefix("TAG:") {
            if let Some((key, val)) = rest.split_once('=') {
                let key = key.to_ascii_uppercase();
                let val = val.trim();
                if val.is_empty() {
                    continue;
                }
                match key.as_str() {
                    "TITLE" if meta.title.is_none() => meta.title = Some(val.to_string()),
                    "ARTIST" if meta.artist.is_none() => meta.artist = Some(val.to_string()),
                    "ALBUM" if meta.album.is_none() => meta.album = Some(val.to_string()),
                    "GENRE" if meta.genre.is_none() => meta.genre = Some(val.to_string()),
                    "DATE" | "YEAR" if meta.year.is_none() => {
                        if let Ok(year) = val.chars().take(4).collect::<String>().parse::<i32>() {
                            meta.year = Some(year);
                        }
                    }
                    "TRACK" if meta.track_number.is_none() => {
                        if let Ok(track_num) = val.split('/').next().unwrap_or("").trim().parse::<i32>() {
                            meta.track_number = Some(track_num);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    Some(meta)
}

/// Decodes an audio stream from any media file (including video containers like MP4, MKV, AVI, etc.)
/// by piping raw 32-bit floating-point PCM audio directly from ffmpeg into on_pcm_chunk.
pub fn decode_track_streaming_ffmpeg<F, C>(
    path: &Path,
    target_sample_rate: u32,
    target_channels: u16,
    mut should_continue: C,
    mut on_pcm_chunk: F,
) -> Result<DecodedTrack>
where
    C: FnMut() -> bool,
    F: FnMut(&[f32]),
{
    let probed = probe_file(path).unwrap_or_default();

    let default_title = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("Unknown title")
        .to_owned();

    let mut metadata = TrackMetadata {
        title: probed.title.unwrap_or(default_title),
        artist: probed.artist.unwrap_or_else(|| "Unknown Artist".to_owned()),
        album: probed.album.unwrap_or_else(|| "Unknown Album".to_owned()),
        duration: probed.duration,
    };

    let out_rate = if target_sample_rate > 0 {
        target_sample_rate
    } else {
        probed.sample_rate.unwrap_or(48000)
    };

    let out_channels = if target_channels > 0 {
        target_channels
    } else {
        probed.channels.unwrap_or(2)
    };

    let mut child = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-nostdin",
            "-i",
        ])
        .arg(path)
        .args([
            "-vn", // ignore video streams
            "-sn", // ignore subtitle streams
            "-dn", // ignore data streams
            "-f",
            "f32le", // 32-bit float Little Endian PCM
            "-ar",
            &out_rate.to_string(),
            "-ac",
            &out_channels.to_string(),
            "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("unable to spawn ffmpeg to decode {}", path.display()))?;

    let mut stdout = child
        .stdout
        .take()
        .context("failed to capture ffmpeg stdout pipe")?;

    let mut byte_buf = [0u8; 16384]; // 4096 f32 samples per read
    let mut carry = Vec::new();
    let mut all_samples = Vec::new();

    while should_continue() {
        let n = match stdout.read(&mut byte_buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.into());
            }
        };

        carry.extend_from_slice(&byte_buf[..n]);
        let float_count = carry.len() / 4;
        if float_count > 0 {
            let mut f32_chunk = Vec::with_capacity(float_count);
            for i in 0..float_count {
                let bytes = [
                    carry[i * 4],
                    carry[i * 4 + 1],
                    carry[i * 4 + 2],
                    carry[i * 4 + 3],
                ];
                f32_chunk.push(f32::from_le_bytes(bytes));
            }
            let remainder = carry[float_count * 4..].to_vec();
            carry = remainder;

            on_pcm_chunk(&f32_chunk);
            all_samples.extend_from_slice(&f32_chunk);
        }
    }

    if !should_continue() {
        let _ = child.kill();
    }
    let _ = child.wait();

    if all_samples.is_empty() {
        anyhow::bail!("ffmpeg did not decode any audio frames from {}", path.display());
    }

    if metadata.duration.is_none() {
        let ch = usize::from(out_channels).max(1);
        let total_frames = all_samples.len() / ch;
        metadata.duration = Some(Duration::from_secs_f64(total_frames as f64 / f64::from(out_rate)));
    }

    Ok(DecodedTrack {
        metadata,
        samples: all_samples,
        sample_rate: out_rate,
        channels: out_channels,
    })
}
