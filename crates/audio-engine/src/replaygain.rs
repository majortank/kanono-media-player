use std::{path::{Path, PathBuf}, thread};

use anyhow::{bail, Context, Result};
use crossbeam_channel::{bounded, Receiver, Sender};
use ebur128::{EbuR128, Mode};
use id3::{frame::{Content, ExtendedText}, Frame, Tag, TagLike, Version};

use crate::decode_track;

pub const REPLAYGAIN_REFERENCE_LUFS: f64 = -18.0;

#[derive(Debug, Clone, Copy)]
pub struct ReplayGainResult {
    pub integrated_lufs: f64,
    pub gain_db: f64,
    pub true_peak: f64,
}

#[derive(Debug, Clone)]
pub enum ReplayGainEvent {
    Complete { path: PathBuf, result: ReplayGainResult },
    Failed { path: PathBuf, error: String },
}

/// Submit files here; the dedicated worker decodes, analyzes, and writes tags.
pub struct ReplayGainWorker {
    jobs: Sender<PathBuf>,
    events: Receiver<ReplayGainEvent>,
}

impl ReplayGainWorker {
    pub fn spawn() -> Self {
        let (jobs, job_receiver) = bounded::<PathBuf>(32);
        let (event_sender, events) = bounded::<ReplayGainEvent>(64);
        thread::Builder::new().name("kanono-replaygain".into()).spawn(move || {
            while let Ok(path) = job_receiver.recv() {
                let event = match analyze_and_tag(&path) {
                    Ok(result) => ReplayGainEvent::Complete { path, result },
                    Err(error) => ReplayGainEvent::Failed { path, error: error.to_string() },
                };
                let _ = event_sender.send(event);
            }
        }).expect("failed to start ReplayGain worker");
        Self { jobs, events }
    }

    pub fn enqueue(&self, path: impl Into<PathBuf>) -> Result<()> {
        self.jobs.try_send(path.into()).map_err(|error| anyhow::anyhow!("ReplayGain job queue unavailable: {error}"))
    }

    pub fn events(&self) -> &Receiver<ReplayGainEvent> {
        &self.events
    }
}

pub fn analyze_and_tag(path: impl AsRef<Path>) -> Result<ReplayGainResult> {
    let path = path.as_ref();
    let decoded = decode_track(path)?;
    let result = calculate_replaygain(&decoded.samples, decoded.channels, decoded.sample_rate)?;
    let _ = write_replaygain_tags(path, result);
    Ok(result)
}

/// Calculates BS.1770 K-weighted, gated integrated loudness and true peak.
pub fn calculate_replaygain(samples: &[f32], channels: u16, sample_rate: u32) -> Result<ReplayGainResult> {
    if channels == 0 || samples.is_empty() || !samples.len().is_multiple_of(usize::from(channels)) {
        bail!("PCM must contain complete interleaved frames");
    }
    let mut analyzer = EbuR128::new(u32::from(channels), sample_rate, Mode::I | Mode::SAMPLE_PEAK)?;
    analyzer.add_frames_f32(samples)?;
    let integrated_lufs = analyzer.loudness_global()?;
    let true_peak = (0..channels)
        .map(|channel| analyzer.sample_peak(u32::from(channel)))
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .fold(0.0_f64, f64::max);
    Ok(ReplayGainResult {
        integrated_lufs,
        gain_db: REPLAYGAIN_REFERENCE_LUFS - integrated_lufs,
        true_peak,
    })
}

fn write_replaygain_tags(path: &Path, result: ReplayGainResult) -> Result<()> {
    if !path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("mp3")) {
        bail!("ReplayGain tag writing currently supports MP3 only: {}", path.display());
    }
    let mut tag = Tag::read_from_path(path).unwrap_or_else(|_| Tag::new());
    tag.add_frame(Frame::with_content("TXXX", Content::ExtendedText(ExtendedText {
        description: "REPLAYGAIN_TRACK_GAIN".into(), value: format!("{:+.2} dB", result.gain_db),
    })));
    tag.add_frame(Frame::with_content("TXXX", Content::ExtendedText(ExtendedText {
        description: "REPLAYGAIN_TRACK_PEAK".into(), value: format!("{:.8}", result.true_peak),
    })));
    tag.write_to_path(path, Version::Id3v24).with_context(|| format!("failed to write ReplayGain tags to {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculates_finite_loudness_and_peak() {
        let samples = vec![0.1_f32; 48_000 * 2];
        let result = calculate_replaygain(&samples, 2, 48_000).unwrap();
        assert!(result.integrated_lufs.is_finite());
        assert!(result.gain_db.is_finite());
        assert!(result.true_peak > 0.0);
    }
}