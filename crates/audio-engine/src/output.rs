use std::{sync::{atomic::{AtomicU32, AtomicU64, Ordering}, Arc}, thread, time::Duration};

use anyhow::{bail, Context, Result};
use cpal::{traits::{DeviceTrait, HostTrait, StreamTrait}, BufferSize, SampleFormat, Stream, StreamConfig};
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
        let mut count = 0usize;
        for sample in samples {
            count += 1;
            if count.is_multiple_of(512) && !should_continue() {
                return false;
            }
            while self.0.push(sample).is_err() {
                if !should_continue() {
                    return false;
                }
                thread::sleep(Duration::from_millis(2));
            }
        }
        true
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn clear(&self) {
        while self.0.pop().is_some() {}
    }

    /// The allocation-free PCM transfer used by the CPAL output callback.
    /// Returns the number of samples actually drained from the queue.
    #[inline]
    pub fn fill_output(&self, output: &mut [f32]) -> usize {
        let mut popped = 0usize;
        for sample in output.iter_mut() {
            if let Some(val) = self.0.pop() {
                *sample = val;
                popped += 1;
            } else {
                *sample = 0.0;
            }
        }
        popped
    }

    #[inline]
    pub fn fill_output_i16(&self, sample: &mut i16) -> i16 {
        let value = self.0.pop().unwrap_or(0.0).clamp(-1.0, 1.0);
        let pcm = if value < 0.0 {
            (value * 32768.0).round().clamp(-32768.0, 32767.0) as i16
        } else {
            (value * 32767.0).round().clamp(-32768.0, 32767.0) as i16
        };
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
    pub volume: Arc<AtomicU32>,
    pub last_position_update: Arc<AtomicU64>,
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
        let (mut config, sample_format) = if let Ok(default_cfg) = device.default_output_config() {
            let format = default_cfg.sample_format();
            (default_cfg.into(), format)
        } else {
            let configs: Vec<_> = device.supported_output_configs().context("failed to inspect output configurations")?.collect();
            let preferred = configs
                .iter()
                .find(|config| config.channels() == 2 && config.sample_format() == SampleFormat::F32)
                .or_else(|| configs.iter().find(|config| config.channels() == 2 && matches!(config.sample_format(), SampleFormat::I16 | SampleFormat::U16)))
                .or_else(|| configs.iter().find(|config| config.sample_format() == SampleFormat::F32))
                .or_else(|| configs.first())
                .context("default output device has no usable stream configuration")?;
            let sample_format = preferred.sample_format();
            let sample_rate = if 44100 >= preferred.min_sample_rate().0 && 44100 <= preferred.max_sample_rate().0 {
                cpal::SampleRate(44100)
            } else if 48000 >= preferred.min_sample_rate().0 && 48000 <= preferred.max_sample_rate().0 {
                cpal::SampleRate(48000)
            } else {
                preferred.min_sample_rate()
            };
            (preferred.with_sample_rate(sample_rate).config(), sample_format)
        };

        if let LatencyProfile::FixedFrames(frames) = latency {
            config.buffer_size = BufferSize::Fixed(frames);
        }
        let queue = SampleQueue::with_capacity(usize::from(config.channels) * config.sample_rate.0 as usize * 10);
        let callback_queue = queue.clone();
        let clock = PlaybackClock::default();
        let callback_clock = clock.clone();
        let volume = Arc::new(AtomicU32::new(1.0_f32.to_bits()));
        let callback_volume = Arc::clone(&volume);
        let last_position_update = Arc::new(AtomicU64::new(0));
        let callback_last_position_update = Arc::clone(&last_position_update);
        let channels = usize::from(config.channels);
        let sample_rate = config.sample_rate.0;
        let stream = match sample_format {
            SampleFormat::F32 => device.build_output_stream(
                &config,
                move |output: &mut [f32], _| {
                    let popped = callback_queue.fill_output(output);
                    let vol = f32::from_bits(callback_volume.load(Ordering::Relaxed));
                    if (vol - 1.0).abs() > 0.001 {
                        for sample in output.iter_mut() {
                            *sample *= vol;
                        }
                    }
                    let actual_frames = (popped / channels) as u64;
                    if actual_frames > 0 {
                        let played_frames = callback_clock.advance(actual_frames);
                        let update_interval = u64::from(sample_rate / 30).max(1);
                        let last_update = callback_last_position_update.load(Ordering::Relaxed);
                        if last_update == 0 || played_frames < last_update || played_frames.saturating_sub(last_update) >= update_interval {
                            callback_last_position_update.store(played_frames, Ordering::Relaxed);
                            state_sender.publish_position(callback_clock.position(sample_rate));
                            let win = output.len().min(512);
                            if win > 0 {
                                state_sender.publish_visualizer_pcm(Arc::from(&output[..win]));
                            }
                        }
                    }
                },
                move |error| eprintln!("audio output error: {error}"),
                None,
            )?,
            SampleFormat::I16 => device.build_output_stream(
                &config,
                move |output: &mut [i16], _| {
                    let vol = f32::from_bits(callback_volume.load(Ordering::Relaxed));
                    let mut popped = 0usize;
                    for sample in output.iter_mut() {
                        if let Some(val) = callback_queue.0.pop() {
                            popped += 1;
                            let value = (val.clamp(-1.0, 1.0) * vol).clamp(-1.0, 1.0);
                            let pcm = if value < 0.0 {
                                (value * 32768.0).round().clamp(-32768.0, 32767.0) as i16
                            } else {
                                (value * 32767.0).round().clamp(-32768.0, 32767.0) as i16
                            };
                            *sample = pcm;
                        } else {
                            *sample = 0;
                        }
                    }
                    let actual_frames = (popped / channels) as u64;
                    if actual_frames > 0 {
                        let played_frames = callback_clock.advance(actual_frames);
                        let update_interval = u64::from(sample_rate / 30).max(1);
                        let last_update = callback_last_position_update.load(Ordering::Relaxed);
                        if last_update == 0 || played_frames < last_update || played_frames.saturating_sub(last_update) >= update_interval {
                            callback_last_position_update.store(played_frames, Ordering::Relaxed);
                            state_sender.publish_position(callback_clock.position(sample_rate));
                            let win = output.len().min(512);
                            if win > 0 {
                                let pcm_f32: Vec<f32> = output[..win].iter().map(|&s| s as f32 / 32768.0).collect();
                                state_sender.publish_visualizer_pcm(Arc::from(pcm_f32.as_slice()));
                            }
                        }
                    }
                },
                move |error| eprintln!("audio output error: {error}"),
                None,
            )?,
            SampleFormat::U16 => device.build_output_stream(
                &config,
                move |output: &mut [u16], _| {
                    let vol = f32::from_bits(callback_volume.load(Ordering::Relaxed));
                    let mut popped = 0usize;
                    for sample in output.iter_mut() {
                        if let Some(val) = callback_queue.0.pop() {
                            popped += 1;
                            let value = val.clamp(-1.0, 1.0) * vol;
                            *sample = ((value + 1.0) * u16::MAX as f32 / 2.0).round() as u16;
                        } else {
                            *sample = 32768;
                        }
                    }
                    let actual_frames = (popped / channels) as u64;
                    if actual_frames > 0 {
                        let played_frames = callback_clock.advance(actual_frames);
                        let update_interval = u64::from(sample_rate / 30).max(1);
                        let last_update = callback_last_position_update.load(Ordering::Relaxed);
                        if last_update == 0 || played_frames < last_update || played_frames.saturating_sub(last_update) >= update_interval {
                            callback_last_position_update.store(played_frames, Ordering::Relaxed);
                            state_sender.publish_position(callback_clock.position(sample_rate));
                            let win = output.len().min(512);
                            if win > 0 {
                                let pcm_f32: Vec<f32> = output[..win].iter().map(|&s| (s as f32 / 32767.5) - 1.0).collect();
                                state_sender.publish_visualizer_pcm(Arc::from(pcm_f32.as_slice()));
                            }
                        }
                    }
                },
                move |error| eprintln!("audio output error: {error}"),
                None,
            )?,
            sample_format => bail!("unsupported audio format: {sample_format:?}"),
        };
        let _ = stream.pause();
        Ok(Self { _stream: stream, queue, config, clock, volume, last_position_update })
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

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn played_frames(&self) -> u64 {
        self.clock.played_frames()
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
        self.last_position_update.store(0, Ordering::Relaxed);
    }

    pub fn set_position(&self, position: Duration) {
        let frames = (position.as_secs_f64() * f64::from(self.config.sample_rate.0)) as u64;
        self.clock.set_frames(frames);
        self.last_position_update.store(frames, Ordering::Relaxed);
    }

    pub fn set_volume(&self, vol: f32) {
        self.volume.store(vol.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    pub fn volume(&self) -> f32 {
        f32::from_bits(self.volume.load(Ordering::Relaxed))
    }
}