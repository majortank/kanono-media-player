use std::{sync::{Arc, Mutex}, thread};

use crossbeam_channel::{bounded, Receiver, Sender, TrySendError};
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
pub struct MprisState {
    inner: Arc<Mutex<MprisStateInner>>,
}

#[derive(Default)]
struct MprisStateInner {
    metadata: TrackMetadata,
    playing: bool,
}

impl MprisState {
    pub fn set_metadata(&self, metadata: TrackMetadata) {
        self.inner.lock().expect("MPRIS state poisoned").metadata = metadata;
    }

    pub fn set_playing(&self, playing: bool) {
        self.inner.lock().expect("MPRIS state poisoned").playing = playing;
    }
}

pub struct MprisService {
    pub commands: Receiver<MprisCommand>,
    pub state: MprisState,
}

impl MprisService {
    pub fn spawn() -> Self {
        let (sender, commands) = bounded(16);
        let state = MprisState::default();
        let service_state = state.clone();
        thread::Builder::new().name("kanono-mpris".into()).spawn(move || {
            let connection = match zbus::blocking::Connection::session() {
                Ok(connection) => connection,
                Err(error) => {
                    eprintln!("MPRIS unavailable: {error}");
                    return;
                }
            };
            if let Err(error) = connection.request_name("org.mpris.MediaPlayer2.kanono") {
                eprintln!("MPRIS name unavailable: {error}");
                return;
            }
            let root = MprisRoot;
            let player = MprisPlayer { commands: sender, state: service_state };
            let object_server = connection.object_server();
            if let Err(error) = object_server.at("/org/mpris/MediaPlayer2", root)
                .and_then(|_| object_server.at("/org/mpris/MediaPlayer2", player)) {
                eprintln!("MPRIS registration failed: {error}");
                return;
            }
            loop { thread::park(); }
        }).expect("failed to start MPRIS service");
        Self { commands, state }
    }
}

struct MprisRoot;

struct MprisPlayer {
    commands: Sender<MprisCommand>,
    state: MprisState,
}

#[zbus::interface(name = "org.mpris.MediaPlayer2")]
impl MprisRoot {
    #[zbus(property)]
    fn can_quit(&self) -> bool { false }

    #[zbus(property)]
    fn can_raise(&self) -> bool { false }

    #[zbus(property)]
    fn has_track_list(&self) -> bool { false }

    #[zbus(property)]
    fn identity(&self) -> &str { "Kanono Media Player" }

    #[zbus(property)]
    fn supported_uri_schemes(&self) -> Vec<&str> { vec!["file"] }

    #[zbus(property)]
    fn supported_mime_types(&self) -> Vec<&str> { vec!["audio/mpeg", "audio/flac", "audio/wav"] }
}

#[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
impl MprisPlayer {
    fn next(&self) { self.send(MprisCommand::Next); }
    fn previous(&self) { self.send(MprisCommand::Previous); }
    fn pause(&self) { self.send(MprisCommand::Pause); }
    fn play_pause(&self) {
        if self.state.inner.lock().expect("MPRIS state poisoned").playing { self.send(MprisCommand::Pause); }
        else { self.send(MprisCommand::Play); }
    }
    fn stop(&self) { self.send(MprisCommand::Stop); }
    fn play(&self) { self.send(MprisCommand::Play); }

    #[zbus(property)]
    fn playback_status(&self) -> &str {
        if self.state.inner.lock().expect("MPRIS state poisoned").playing { "Playing" } else { "Stopped" }
    }

    #[zbus(property)]
    fn can_go_next(&self) -> bool { true }

    #[zbus(property)]
    fn can_go_previous(&self) -> bool { true }

    #[zbus(property)]
    fn can_play(&self) -> bool { true }

    #[zbus(property)]
    fn can_pause(&self) -> bool { true }

    #[zbus(property)]
    fn can_control(&self) -> bool { true }
}

impl MprisPlayer {
    fn send(&self, command: MprisCommand) {
        match self.commands.try_send(command) {
            Ok(()) | Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {}
        }
    }
}