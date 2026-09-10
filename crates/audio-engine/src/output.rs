use std::{sync::{atomic::{AtomicU64, Ordering}, Arc}, thread, time::Duration};

use anyhow::{bail, Context, Result};
use cpal::{traits::{DeviceTrait, HostTrait, StreamTrait}, BufferSize, SampleFormat, Stream, StreamConfig, SupportedBufferSize};
use crossbeam_queue::ArrayQueue;

use crate::playback_state::{PlaybackClock, PlaybackStateSender};

#[cfg(test)]
mod tests {
    use super::SampleQueue;

    #[test]
    fn converts_f32_ring_to_i16_without_clipping() {
        let queue = SampleQueue::default();
        let mut out = vec![0_i16; 4];
        queue.push_interleaved([0.0, 1.0, -1.0, 0.5]);
        for sample in out.iter_mut() {
            let value = queue.fill_output_i16(sample);
            *sample = value;
        }
        assert_eq!(out[0], 0);
        assert_eq!(out[1], i16::MAX);
        assert_eq!(out[2], i16::MIN);
        assert!(out[3] > 0);
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub enum LatencyProfile {
    #[default]
    Default,
    FixedFrames(u32),
}

/// Interleaved 32-bit floating point samples shared by the decode worker and callback.
#[derive(Clone)]
pub struct SampleQueue(Arc<ArrayQueue<f32>>);

impl Default for SampleQueue {
    fn default() -> Self {
        Self::with_capacity(48_000 * 2 * 10)
    }
}

impl SampleQueue {
    pub fn with_capacity(capacity: usize) -> Self {
        Self(Arc::new(ArrayQueue::new(capacity)))
    }

    /// Blocks only the producer when the fixed PCM ring is full; never call from CPAL.
    pub fn push_interleaved(&self, samples: impl IntoIterator<Item = f32>) {
        self.push_interleaved_cancellable(samples, || true);
    }

    /// Returns false when playback was superseded while a decode worker was filling the ring.
    pub fn push_interleaved_cancellable(
        &self,
        samples: impl IntoIterator<Item = f32>,
        mut should_continue: impl FnMut() -> bool,
    ) -> bool {
        for sample in samples {
            while self.0.push(sample).is_err() {
                if !should_continue() {
                    return false;
                }
                thread::yield_now();
            }
        }
        true
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn clear(&self) {
        while self.0.pop().is_some() {}
    }

    /// The allocation-free PCM transfer used by the CPAL output callback.
    #[inline]
    pub fn fill_output(&self, output: &mut [f32]) {
        for sample in output.iter_mut() {
            *sample = self.0.pop().unwrap_or(0.0);
        }
    }

    #[inline]
    pub fn fill_output_i16(&self, sample: &mut i16) -> i16 {
        let value = self.0.pop().unwrap_or(0.0).clamp(-1.0, 1.0);
        let pcm = (value * i16::MAX as f32).round() as i16;
        *sample = pcm;
        pcm
    }
}

/// Keeps the CPAL stream alive. The callback does no decoding or allocation.
pub struct AudioOutput {
    _stream: Stream,
    pub queue: SampleQueue,
    pub config: StreamConfig,
    pub clock: PlaybackClock,
}

impl AudioOutput {
    pub fn open_default(state_sender: PlaybackStateSender) -> Result<Self> {
        Self::open_default_tuned(state_sender, LatencyProfile::Default)
    }

    /// Requests a hardware buffer size for PipeWire's ALSA compatibility layer or JACK.
    /// The audio server remains authoritative and may reject unavailable fixed sizes.
    pub fn open_default_tuned(state_sender: PlaybackStateSender, latency: LatencyProfile) -> Result<Self> {
        let device = cpal::default_host()
            .default_output_device()
            .context("no default audio output device")?;
        let configs: Vec<_> = device.supported_output_configs().context("failed to inspect output configurations")?.collect();
        let preferred = configs
            .iter()
            .find(|config| config.sample_format() == SampleFormat::F32)
            .or_else(|| configs.iter().find(|config| matches!(config.sample_format(), SampleFormat::I16 | SampleFormat::U16)))
            .context("default output device has no usable stream configuration")?;
        let sample_format = preferred.sample_format();
        let mut config = preferred.with_max_sample_rate().config();
        if let LatencyProfile::FixedFrames(frames) = latency {
            match preferred.buffer_size() {
                SupportedBufferSize::Range { min, max } if frames >= *min && frames <= *max => {
                    config.buffer_size = BufferSize::Fixed(frames);
                }
                SupportedBufferSize::Range { min, max } => {
                    bail!("requested {frames} frames, but output supports {min} through {max}");
                }
                SupportedBufferSize::Unknown => {
                    bail!("output device does not report fixed buffer-size support");
                }
            }
        }
        let queue = SampleQueue::with_capacity(usize::from(config.channels) * config.sample_rate.0 as usize * 10);
        let callback_queue = queue.clone();
        let clock = PlaybackClock::default();
        let callback_clock = clock.clone();
        let last_position_update = Arc::new(AtomicU64::new(0));
        let callback_last_position_update = Arc::clone(&last_position_update);
        let channels = usize::from(config.channels);
        let sample_rate = config.sample_rate.0;
        let stream = match sample_format {
            SampleFormat::F32 => device.build_output_stream(
                &config,
                move |output: &mut [f32], _| {
                    let frames = output.len() / channels;
                    callback_queue.fill_output(output);
                    let played_frames = callback_clock.advance(frames as u64);
                    let update_interval = u64::from(sample_rate / 30).max(1);
                    let last_update = callback_last_position_update.load(Ordering::Relaxed);
                    if played_frames.saturating_sub(last_update) >= update_interval {
                        callback_last_position_update.store(played_frames, Ordering::Relaxed);
                        state_sender.publish_position(callback_clock.position(sample_rate));
                    }
                },
                move |error| eprintln!("audio output error: {error}"),
                None,
            )?,
            SampleFormat::I16 => device.build_output_stream(
                &config,
                move |output: &mut [i16], _| {
                    let frames = output.len() / channels;
                    for sample in output.iter_mut() {
                        let value = callback_queue.0.pop().unwrap_or(0.0).clamp(-1.0, 1.0);
                        *sample = (value * i16::MAX as f32).round() as i16;
                    }
                    let played_frames = callback_clock.advance(frames as u64);
                    let update_interval = u64::from(sample_rate / 30).max(1);
                    let last_update = callback_last_position_update.load(Ordering::Relaxed);
                    if played_frames.saturating_sub(last_update) >= update_interval {
                        callback_last_position_update.store(played_frames, Ordering::Relaxed);
                        state_sender.publish_position(callback_clock.position(sample_rate));
                    }
                },
                move |error| eprintln!("audio output error: {error}"),
                None,
            )?,
            SampleFormat::U16 => device.build_output_stream(
                &config,
                move |output: &mut [u16], _| {
                    let frames = output.len() / channels;
                    for sample in output.iter_mut() {
                        let value = callback_queue.0.pop().unwrap_or(0.0).clamp(-1.0, 1.0);
                        *sample = ((value + 1.0) * u16::MAX as f32 / 2.0).round() as u16;
                    }
                    let played_frames = callback_clock.advance(frames as u64);
                    let update_interval = u64::from(sample_rate / 30).max(1);
                    let last_update = callback_last_position_update.load(Ordering::Relaxed);
                    if played_frames.saturating_sub(last_update) >= update_interval {
                        callback_last_position_update.store(played_frames, Ordering::Relaxed);
                        state_sender.publish_position(callback_clock.position(sample_rate));
                    }
                },
                move |error| eprintln!("audio output error: {error}"),
                None,
            )?,
            sample_format => bail!("unsupported audio format: {sample_format:?}"),
        };
        Ok(Self { _stream: stream, queue, config, clock })
    }

    /// Append a decoded track without clearing queued samples to preserve gapless order.
    pub fn enqueue_track(&self, interleaved_f32: Vec<f32>) -> Result<()> {
        if interleaved_f32.is_empty() {
            bail!("decoded track contained no samples");
        }
        self.queue.push_interleaved(interleaved_f32);
        Ok(())
    }

    pub fn elapsed(&self) -> Duration {
        self.clock.position(self.config.sample_rate.0)
    }

    pub fn play(&self) -> Result<()> {
        self._stream.play()?;
        Ok(())
    }

    pub fn pause(&self) -> Result<()> {
        self._stream.pause()?;
        Ok(())
    }

    pub fn reset_position(&self) {
        self.clock.reset();
    }
}