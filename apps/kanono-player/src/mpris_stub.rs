use crossbeam_channel::{bounded, Receiver};
use kanono_audio_engine::TrackMetadata;

#[derive(Debug, Clone, Copy)]
pub enum MprisCommand {
    Play,
    Pause,
    Stop,
    Next,
    Previous,
}

#[derive(Clone, Default)]
pub struct MprisState;

impl MprisState {
    pub fn set_metadata(&self, _metadata: TrackMetadata) {}

    pub fn set_playing(&self, _playing: bool) {}
}

pub struct MprisService {
    pub commands: Receiver<MprisCommand>,
    pub state: MprisState,
}

impl MprisService {
    pub fn spawn() -> Self {
        let (_sender, commands) = bounded(1);
        Self {
            commands,
            state: MprisState,
        }
    }
}