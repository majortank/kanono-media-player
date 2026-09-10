use std::{sync::{atomic::{AtomicU64, Ordering}, Arc}, time::Duration};

use crossbeam_channel::{bounded, Receiver, Sender, TrySendError};

pub const VISUALIZER_WINDOW_SAMPLES: usize = 2_048;

#[derive(Debug, Clone, Default)]
pub struct TrackMetadata {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: Option<Duration>,
}

#[derive(Debug, Clone)]
pub enum PlaybackUpdate {
    Track(TrackMetadata),
    Position(Duration),
    VisualizerPcm(Arc<[f32]>),
}

/// Sender held by audio/decode workers. Full queues deliberately discard stale UI data.
#[derive(Clone)]
pub struct PlaybackStateSender {
    sender: Sender<PlaybackUpdate>,
}

pub struct PlaybackStateReceiver {
    receiver: Receiver<PlaybackUpdate>,
}

pub fn playback_state_channel() -> (PlaybackStateSender, PlaybackStateReceiver) {
    let (sender, receiver) = bounded(64);
    (PlaybackStateSender { sender }, PlaybackStateReceiver { receiver })
}

impl PlaybackStateSender {
    pub fn publish_track(&self, metadata: TrackMetadata) {
        self.send(PlaybackUpdate::Track(metadata));
    }

    pub fn publish_visualizer_pcm(&self, samples: Arc<[f32]>) {
        self.send(PlaybackUpdate::VisualizerPcm(samples));
    }

    pub fn publish_position(&self, position: Duration) {
        self.send(PlaybackUpdate::Position(position));
    }

    fn send(&self, update: PlaybackUpdate) {
        match self.sender.try_send(update) {
            Ok(()) | Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {}
        }
    }

}

impl PlaybackStateReceiver {
    /// Drains pending updates so the UI always renders the most recent state.
    pub fn drain(&self) -> Vec<PlaybackUpdate> {
        self.receiver.try_iter().collect()
    }

    /// Discards all pending updates in the channel.
    pub fn clear(&self) {
        while self.receiver.try_recv().is_ok() {}
    }
}

/// Sample-accurate position shared from CPAL's callback without channel traffic.
#[derive(Clone, Default)]
pub struct PlaybackClock(Arc<AtomicU64>);

impl PlaybackClock {
    pub fn advance(&self, frames: u64) -> u64 {
        self.0.fetch_add(frames, Ordering::Relaxed) + frames
    }

    pub fn position(&self, sample_rate: u32) -> Duration {
        Duration::from_secs_f64(self.0.load(Ordering::Relaxed) as f64 / f64::from(sample_rate))
    }

    pub fn reset(&self) {
        self.0.store(0, Ordering::Relaxed);
    }

    pub fn set_frames(&self, frames: u64) {
        self.0.store(frames, Ordering::Relaxed);
    }

    pub fn played_frames(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}