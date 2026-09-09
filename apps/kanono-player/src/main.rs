mod mpris;
mod plugins;
mod ui;

use std::{path::PathBuf, sync::{atomic::{AtomicU64, Ordering}, Arc}, thread, time::Duration};

use iced::{executor, time, Application, Command, Element, Subscription, Theme};
use kanono_audio_engine::{decode_file, playback_state_channel, AudioOutput, LibraryDatabase, LibraryTrack, PlaybackStateReceiver, PlaybackStateSender, PlaybackUpdate, TrackMetadata, TrackQuery};
use mpris::{MprisCommand, MprisService, MprisState};
use plugins::PluginRegistry;
use ui::player_view;

fn main() -> iced::Result {
    KanonoApp::run(iced::Settings::default())
}

struct KanonoApp {
    playback: PlaybackViewState,
    playback_sender: PlaybackStateSender,
    playback_receiver: PlaybackStateReceiver,
    audio_output: Option<AudioOutput>,
    playback_generation: Arc<AtomicU64>,
    mpris_commands: crossbeam_channel::Receiver<MprisCommand>,
    mpris_state: MprisState,
    components: PluginRegistry,
    all_tracks: Vec<LibraryTrack>,
    visible_tracks: Vec<LibraryTrack>,
    selected_folder: Option<PathBuf>,
    search: String,
    selected_track: Option<i64>,
    queued_track_ids: Vec<i64>,
    is_playing: bool,
}

#[derive(Default)]
struct PlaybackViewState {
    metadata: TrackMetadata,
    elapsed: Duration,
    visualizer_pcm: Vec<f32>,
}

#[derive(Debug, Clone)]
enum Message {
    PollPlayback,
    PollMpris,
    ImportFolder,
    FolderPicked(Option<PathBuf>),
    LibraryIndexed(IndexResult),
    SearchChanged(String),
    SelectFolder(Option<PathBuf>),
    SelectTrack(i64),
    QueueTrack(i64),
    PlayTrack(i64),
    TogglePlayback,
    Next,
    Previous,
}

impl Application for KanonoApp {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let (playback_sender, playback_receiver) = playback_state_channel();
        let audio_output = match AudioOutput::open_default(playback_sender.clone()) {
            Ok(output) => Some(output),
            Err(error) => {
                eprintln!("audio output unavailable: {error}");
                None
            }
        };
        let components = PluginRegistry::load_components(component_directory());
        let mpris = MprisService::spawn();
        let library = LibraryDatabase::open(library_database_path()).expect("failed to open music library database");
        let all_tracks = library.query(&TrackQuery::default()).unwrap_or_default();
        (
            Self {
                playback: PlaybackViewState::default(),
                playback_sender,
                playback_receiver,
                audio_output,
                playback_generation: Arc::new(AtomicU64::new(0)),
                mpris_commands: mpris.commands,
                mpris_state: mpris.state,
                components,
                visible_tracks: all_tracks.clone(),
                all_tracks,
                selected_folder: None,
                search: String::new(),
                selected_track: None,
                queued_track_ids: Vec::new(),
                is_playing: false,
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        format!(
            "Kanono Media Player - {} components, {} unavailable",
            self.components.plugins().len(),
            self.components.failures().len(),
        )
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::PollPlayback => self.apply_playback_updates(),
            Message::PollMpris => self.apply_mpris_commands(),
            Message::ImportFolder => return Command::perform(pick_music_folder(), Message::FolderPicked),
            Message::FolderPicked(folder) => {
                if let Some(folder) = folder {
                    return Command::perform(index_music_folder(folder), Message::LibraryIndexed);
                }
            }
            Message::LibraryIndexed(result) => self.apply_index_result(result),
            Message::SearchChanged(search) => { self.search = search; self.refresh_visible_tracks(); }
            Message::SelectFolder(folder) => { self.selected_folder = folder; self.refresh_visible_tracks(); }
            Message::SelectTrack(track_id) => self.selected_track = Some(track_id),
            Message::QueueTrack(track_id) => self.queued_track_ids.push(track_id),
            Message::PlayTrack(track_id) => self.play_track(track_id),
            Message::TogglePlayback => { self.is_playing = !self.is_playing; self.mpris_state.set_playing(self.is_playing); }
            Message::Next => self.play_next(),
            Message::Previous => {},
        }
        Command::none()
    }

    fn view(&self) -> Element<'_, Message> {
        player_view(&self.visible_tracks, self.selected_folder.as_ref(), &self.search, self.selected_track, self.queued_track_ids.len(), self.is_playing, &self.playback.metadata, self.playback.elapsed)
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            time::every(Duration::from_millis(33)).map(|_| Message::PollPlayback),
            time::every(Duration::from_millis(100)).map(|_| Message::PollMpris),
        ])
    }
}

#[derive(Debug, Clone)]
struct IndexResult {
    folder: PathBuf,
    tracks: Vec<LibraryTrack>,
    error: Option<String>,
}

async fn pick_music_folder() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new().set_title("Add music folder").pick_folder().await.map(|handle| handle.path().to_owned())
}

async fn index_music_folder(folder: PathBuf) -> IndexResult {
    let database_path = library_database_path();
    let result = tokio::task::spawn_blocking({
        let folder = folder.clone();
        move || -> Result<Vec<LibraryTrack>, String> {
            let mut library = LibraryDatabase::open(database_path).map_err(|error| error.to_string())?;
            library.scan_directory(&folder).map_err(|error| error.to_string())?;
            library.query(&TrackQuery::default()).map_err(|error| error.to_string())
        }
    }).await;
    match result {
        Ok(Ok(tracks)) => IndexResult { folder, tracks, error: None },
        Ok(Err(error)) => IndexResult { folder, tracks: Vec::new(), error: Some(error) },
        Err(error) => IndexResult { folder, tracks: Vec::new(), error: Some(format!("music indexing worker failed: {error}")) },
    }
}

fn component_directory() -> PathBuf {
    std::env::var_os("KANONO_COMPONENTS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("components"))
}

fn library_database_path() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("kanono-media-player/library.sqlite3")
}

impl KanonoApp {
    fn apply_playback_updates(&mut self) {
        for update in self.playback_receiver.drain() {
            match update {
                PlaybackUpdate::Track(metadata) => self.playback.metadata = metadata,
                PlaybackUpdate::Position(position) => self.playback.elapsed = position,
                PlaybackUpdate::VisualizerPcm(samples) => {
                    self.playback.visualizer_pcm = samples.as_ref().to_vec();
                }
            }
        }
        self.mpris_state.set_metadata(self.playback.metadata.clone());
    }

    fn apply_mpris_commands(&mut self) {
        let commands: Vec<_> = self.mpris_commands.try_iter().collect();
        for command in commands {
            match command {
                MprisCommand::Play => { self.is_playing = true; self.mpris_state.set_playing(true); }
                MprisCommand::Pause | MprisCommand::Stop => { self.is_playing = false; self.mpris_state.set_playing(false); }
                MprisCommand::Next => self.play_next(),
                MprisCommand::Previous => {}
            }
        }
    }

    fn apply_index_result(&mut self, result: IndexResult) {
        if let Some(error) = result.error {
            eprintln!("unable to index {}: {error}", result.folder.display());
            return;
        }
        self.all_tracks = result.tracks;
        self.selected_folder = Some(result.folder);
        self.refresh_visible_tracks();
    }

    fn refresh_visible_tracks(&mut self) {
        let search = self.search.to_ascii_lowercase();
        self.visible_tracks = self.all_tracks.iter().filter(|track| {
            let matches_folder = self.selected_folder.as_ref().is_none_or(|folder| track.path.starts_with(folder));
            let haystack = format!("{} {} {} {}", track.title, track.artist, track.album, track.genre).to_ascii_lowercase();
            matches_folder && haystack.contains(&search)
        }).cloned().collect();
    }

    fn play_track(&mut self, track_id: i64) {
        let Some((path, metadata)) = self.all_tracks.iter().find(|track| track.id == track_id).map(|track| {
            (track.path.clone(), TrackMetadata {
                title: track.title.clone(),
                artist: track.artist.clone(),
                album: track.album.clone(),
                duration: track.duration,
            })
        }) else { return };
        self.playback.metadata = metadata;
        self.selected_track = Some(track_id);
        self.is_playing = true;
        self.mpris_state.set_metadata(self.playback.metadata.clone());
        self.mpris_state.set_playing(true);
        self.playback_sender.publish_track(self.playback.metadata.clone());
        if let Some(audio_output) = &self.audio_output {
            let queue = audio_output.queue.clone();
            queue.clear();
            let generation = self.playback_generation.fetch_add(1, Ordering::Relaxed) + 1;
            let playback_generation = Arc::clone(&self.playback_generation);
            thread::spawn(move || match decode_file(&path) {
                Ok(samples) => {
                    queue.push_interleaved_cancellable(samples, || {
                        playback_generation.load(Ordering::Relaxed) == generation
                    });
                }
                Err(error) => eprintln!("unable to decode {}: {error}", path.display()),
            });
        }
    }

    fn play_next(&mut self) {
        if let Some(track_id) = self.queued_track_ids.first().copied() {
            self.queued_track_ids.remove(0);
            self.play_track(track_id);
        }
    }
}