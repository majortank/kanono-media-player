use std::{collections::VecDeque, path::Path};

use anyhow::{bail, Result};

use crate::{decode_track, DecodedTrack, PlaybackStateSender, SampleQueue};

/// Track transition coordinator. All decoding must happen outside CPAL's callback.
pub struct GaplessQueueManager {
    queue: SampleQueue,
    state_sender: PlaybackStateSender,
    prepared: VecDeque<DecodedTrack>,
    low_water_samples: usize,
    started: bool,
}

impl GaplessQueueManager {
    pub fn new(queue: SampleQueue, state_sender: PlaybackStateSender, channels: u16, sample_rate: u32) -> Self {
        Self {
            queue,
            state_sender,
            prepared: VecDeque::new(),
            low_water_samples: usize::from(channels) * sample_rate as usize * 2,
            started: false,
        }
    }

    /// Runs on a decode worker. Once this returns, both metadata and PCM are ready.
    pub fn preload_path(&mut self, path: impl AsRef<Path>) -> Result<()> {
        self.preload_decoded(decode_track(path)?)
    }

    /// Accepts a track from an external decode worker after metadata parsing completes.
    pub fn preload_decoded(&mut self, track: DecodedTrack) -> Result<()> {
        if track.samples.is_empty() {
            bail!("prepared track contained no audio samples");
        }
        self.prepared.push_back(track);
        Ok(())
    }

    /// Starts only when a successor was prepared, unless this is explicitly the final track.
    pub fn start(&mut self, final_track: bool) -> Result<()> {
        if self.started {
            bail!("gapless queue has already started");
        }
        if self.prepared.is_empty() || (!final_track && self.prepared.len() < 2) {
            bail!("prepare the next track metadata and PCM before starting playback");
        }
        self.started = true;
        self.append_next()
    }

    /// Call from a control/decode worker. It never runs in the CPAL callback.
    pub fn promote_if_low_water(&mut self) -> Result<bool> {
        if !self.started || self.queue.len() > self.low_water_samples || self.prepared.is_empty() {
            return Ok(false);
        }
        self.append_next()?;
        Ok(true)
    }

    pub fn prepared_count(&self) -> usize {
        self.prepared.len()
    }

    fn append_next(&mut self) -> Result<()> {
        let track = self.prepared.pop_front().expect("prepared track checked before promotion");
        self.state_sender.publish_track(track.metadata);
        self.queue.push_interleaved(track.samples);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{playback_state_channel, TrackMetadata};

    fn prepared_track(title: &str) -> DecodedTrack {
        DecodedTrack {
            metadata: TrackMetadata { title: title.to_owned(), ..TrackMetadata::default() },
            samples: vec![0.0, 0.0],
        }
    }

    #[test]
    fn non_final_track_requires_prepared_successor_before_starting() {
        let (sender, _receiver) = playback_state_channel();
        let queue = SampleQueue::default();
        let mut manager = GaplessQueueManager::new(queue.clone(), sender, 2, 48_000);
        manager.preload_decoded(prepared_track("current")).unwrap();

        assert!(manager.start(false).is_err());

        manager.preload_decoded(prepared_track("next")).unwrap();
        manager.start(false).unwrap();
        assert_eq!(queue.len(), 2);
        assert_eq!(manager.prepared_count(), 1);
    }
}