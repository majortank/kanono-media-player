use std::{sync::{atomic::{AtomicU64, Ordering}, Arc}, thread, time::Duration};

use anyhow::{bail, Context, Result};
use cpal::{traits::{DeviceTrait, HostTrait, StreamTrait}, BufferSize, SampleFormat, Stream, StreamConfig, SupportedBufferSize};
use crossbeam_queue::ArrayQueue;

use crate::playback_state::{PlaybackClock, PlaybackStateSender};

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
        let supported = device
            .supported_output_configs()
            .context("failed to inspect output configurations")?
            .find(|config| config.sample_format() == SampleFormat::F32)
            .context("default output device has no f32 stream configuration")?;
        let mut config = supported.with_max_sample_rate().config();
        if let LatencyProfile::FixedFrames(frames) = latency {
            match supported.buffer_size() {
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
        let stream = device.build_output_stream(
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
        )?;
        stream.play()?;
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
}