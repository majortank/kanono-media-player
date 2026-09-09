use std::{collections::VecDeque, sync::{Arc, Mutex}, time::Duration};

use anyhow::{bail, Context, Result};
use cpal::{traits::{DeviceTrait, HostTrait, StreamTrait}, SampleFormat, Stream, StreamConfig};

use crate::playback_state::{PlaybackClock, PlaybackStateSender};

/// Interleaved 32-bit floating point samples shared by the decode worker and callback.
#[derive(Clone, Default)]
pub struct SampleQueue(Arc<Mutex<VecDeque<f32>>>);

impl SampleQueue {
    pub fn push_interleaved(&self, samples: impl IntoIterator<Item = f32>) {
        self.0.lock().expect("audio queue poisoned").extend(samples);
    }

    pub fn len(&self) -> usize {
        self.0.lock().expect("audio queue poisoned").len()
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
        let device = cpal::default_host()
            .default_output_device()
            .context("no default audio output device")?;
        let supported = device
            .supported_output_configs()
            .context("failed to inspect output configurations")?
            .find(|config| config.sample_format() == SampleFormat::F32)
            .context("default output device has no f32 stream configuration")?;
        let config = supported.with_max_sample_rate().config();
        let queue = SampleQueue::default();
        let callback_queue = queue.clone();
        let clock = PlaybackClock::default();
        let callback_clock = clock.clone();
        let channels = usize::from(config.channels);
        let sample_rate = config.sample_rate.0;
        let stream = device.build_output_stream(
            &config,
            move |output: &mut [f32], _| {
                let frames = output.len() / channels;
                let mut queued = callback_queue.0.lock().expect("audio queue poisoned");
                for sample in output.iter_mut() {
                    *sample = queued.pop_front().unwrap_or(0.0);
                }
                callback_clock.advance(frames as u64);
                state_sender.publish_position(callback_clock.position(sample_rate));
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