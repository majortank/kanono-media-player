mod mpris;
mod plugins;
mod ui;

use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use iced::{executor, time, Application, Command, Element, Subscription, Theme};
use kanono_audio_engine::{
    decode_track, playback_state_channel, resample_and_remap_channels, AudioOutput,
    LibraryDatabase, LibraryTrack, PlaybackStateReceiver, PlaybackStateSender, PlaybackUpdate,
    TrackMetadata, TrackQuery,
};
use mpris::{MprisCommand, MprisService, MprisState};
use plugins::PluginRegistry;
use ui::{player_view, ViewProps};

fn main() -> iced::Result {
    let mut settings = iced::Settings::default();
    settings.window.size = iced::Size::new(1060.0, 720.0);
    settings.window.min_size = Some(iced::Size::new(800.0, 560.0));
    KanonoApp::run(settings)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NavTab {
    #[default]
    Library,
    Queue,
    Folders,
    Info,
}

pub struct KanonoApp {
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
    current_tab: NavTab,
    search: String,
    selected_track: Option<i64>,
    current_playing_track_id: Option<i64>,
    queued_track_ids: Vec<i64>,
    is_playing: bool,
    volume: f32,
    is_muted: bool,
    prev_volume: f32,
    is_shuffled: bool,
    is_repeated: bool,
    current_track_samples: Option<Arc<Vec<f32>>>,
    status_message: Option<String>,
}

#[derive(Default)]
struct PlaybackViewState {
    metadata: TrackMetadata,
    elapsed: Duration,
    visualizer_pcm: Vec<f32>,
}

#[derive(Debug, Clone)]
pub enum Message {
    PollPlayback,
    PollMpris,
    ImportFolder,
    FolderPicked(Option<PathBuf>),
    LibraryIndexed(IndexResult),
    SearchChanged(String),
    SelectFolder(Option<PathBuf>),
    SelectTab(NavTab),
    SelectTrack(i64),
    QueueTrack(i64),
    RemoveFromQueue(usize),
    ClearQueue,
    PlayTrack(i64),
    TogglePlayback,
    Next,
    Previous,
    Seek(f32),
    VolumeChanged(f32),
    ToggleMute,
    ToggleShuffle,
    ToggleRepeat,
    GenerateSampleAudio,
    SampleAudioGenerated(Result<Vec<LibraryTrack>, String>),
}

impl Application for KanonoApp {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let (playback_sender, playback_receiver) = playback_state_channel();
        let audio_output = match AudioOutput::open_default(playback_sender.clone()) {
            Ok(output) => {
                output.set_volume(0.85);
                Some(output)
            }
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
                current_tab: NavTab::Library,
                search: String::new(),
                selected_track: None,
                current_playing_track_id: None,
                queued_track_ids: Vec::new(),
                is_playing: false,
                volume: 0.85,
                is_muted: false,
                prev_volume: 0.85,
                is_shuffled: false,
                is_repeated: false,
                current_track_samples: None,
                status_message: None,
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        let now_playing = if self.playback.metadata.title.is_empty() {
            "Ready".to_string()
        } else {
            format!("{} - {}", self.playback.metadata.artist, self.playback.metadata.title)
        };
        format!("Kanono Media Player - {}", now_playing)
    }

    fn theme(&self) -> Self::Theme {
        Theme::Dark
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
            Message::SearchChanged(search) => {
                self.search = search;
                self.refresh_visible_tracks();
            }
            Message::SelectFolder(folder) => {
                self.selected_folder = folder;
                self.current_tab = NavTab::Folders;
                self.refresh_visible_tracks();
            }
            Message::SelectTab(tab) => {
                self.current_tab = tab;
                if tab == NavTab::Library {
                    self.selected_folder = None;
                    self.refresh_visible_tracks();
                }
            }
            Message::SelectTrack(track_id) => {
                self.selected_track = Some(track_id);
            }
            Message::QueueTrack(track_id) => {
                self.queued_track_ids.push(track_id);
                self.status_message = Some("Track added to queue".to_string());
            }
            Message::RemoveFromQueue(idx) => {
                if idx < self.queued_track_ids.len() {
                    self.queued_track_ids.remove(idx);
                }
            }
            Message::ClearQueue => {
                self.queued_track_ids.clear();
            }
            Message::PlayTrack(track_id) => {
                self.play_track(track_id);
            }
            Message::TogglePlayback => {
                self.toggle_playback();
            }
            Message::Next => {
                self.play_next();
            }
            Message::Previous => {
                self.play_previous();
            }
            Message::Seek(fraction) => {
                self.seek_to(fraction);
            }
            Message::VolumeChanged(vol) => {
                self.volume = vol;
                self.is_muted = vol == 0.0;
                if let Some(output) = &self.audio_output {
                    output.set_volume(vol);
                }
            }
            Message::ToggleMute => {
                if self.is_muted {
                    self.is_muted = false;
                    let target = if self.prev_volume > 0.05 { self.prev_volume } else { 0.5 };
                    self.volume = target;
                    if let Some(output) = &self.audio_output {
                        output.set_volume(target);
                    }
                } else {
                    self.prev_volume = self.volume;
                    self.is_muted = true;
                    self.volume = 0.0;
                    if let Some(output) = &self.audio_output {
                        output.set_volume(0.0);
                    }
                }
            }
            Message::ToggleShuffle => {
                self.is_shuffled = !self.is_shuffled;
            }
            Message::ToggleRepeat => {
                self.is_repeated = !self.is_repeated;
            }
            Message::GenerateSampleAudio => {
                return Command::perform(create_and_index_demo_tracks(), Message::SampleAudioGenerated);
            }
            Message::SampleAudioGenerated(res) => match res {
                Ok(tracks) => {
                    self.all_tracks = tracks;
                    self.selected_folder = None;
                    self.current_tab = NavTab::Library;
                    self.refresh_visible_tracks();
                    self.status_message = Some("Sample music generated & loaded!".to_string());
                    if let Some(first) = self.visible_tracks.first() {
                        self.play_track(first.id);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to generate sample audio: {e}");
                }
            },
        }
        Command::none()
    }

    fn view(&self) -> Element<'_, Message> {
        player_view(ViewProps {
            tracks: &self.visible_tracks,
            all_tracks: &self.all_tracks,
            selected_folder: self.selected_folder.as_ref(),
            current_tab: self.current_tab,
            search: &self.search,
            selected_track: self.selected_track,
            current_playing_track_id: self.current_playing_track_id,
            queued_track_ids: &self.queued_track_ids,
            is_playing: self.is_playing,
            is_shuffled: self.is_shuffled,
            is_repeated: self.is_repeated,
            volume: self.volume,
            is_muted: self.is_muted,
            metadata: &self.playback.metadata,
            elapsed: self.playback.elapsed,
            status_message: self.status_message.as_deref(),
            component_count: self.components.plugins().len(),
        })
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            time::every(Duration::from_millis(33)).map(|_| Message::PollPlayback),
            time::every(Duration::from_millis(100)).map(|_| Message::PollMpris),
        ])
    }
}

#[derive(Debug, Clone)]
pub struct IndexResult {
    folder: PathBuf,
    tracks: Vec<LibraryTrack>,
    error: Option<String>,
}

async fn pick_music_folder() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title("Add Music Folder")
        .pick_folder()
        .await
        .map(|handle| handle.path().to_owned())
}

async fn index_music_folder(folder: PathBuf) -> IndexResult {
    let database_path = library_database_path();
    let result = tokio::task::spawn_blocking({
        let folder = folder.clone();
        move || -> Result<Vec<LibraryTrack>, String> {
            let mut library = LibraryDatabase::open(database_path).map_err(|e| e.to_string())?;
            library.scan_directory(&folder).map_err(|e| e.to_string())?;
            library.query(&TrackQuery::default()).map_err(|e| e.to_string())
        }
    })
    .await;

    match result {
        Ok(Ok(tracks)) => IndexResult { folder, tracks, error: None },
        Ok(Err(error)) => IndexResult { folder, tracks: Vec::new(), error: Some(error) },
        Err(error) => IndexResult {
            folder,
            tracks: Vec::new(),
            error: Some(format!("music indexing worker failed: {error}")),
        },
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

        // Automatic track progression when song finishes
        if self.is_playing {
            if let Some(dur) = self.playback.metadata.duration {
                if dur.as_secs() > 0 && self.playback.elapsed >= dur {
                    self.play_next();
                }
            }
        }
    }

    fn apply_mpris_commands(&mut self) {
        let commands: Vec<_> = self.mpris_commands.try_iter().collect();
        for command in commands {
            match command {
                MprisCommand::Play => self.resume_playback(),
                MprisCommand::Pause => self.pause_playback(),
                MprisCommand::Stop => self.stop_playback(),
                MprisCommand::Next => self.play_next(),
                MprisCommand::Previous => self.play_previous(),
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
        self.visible_tracks = self
            .all_tracks
            .iter()
            .filter(|track| {
                let matches_folder = self
                    .selected_folder
                    .as_ref()
                    .map(|folder| track.path.starts_with(folder))
                    .unwrap_or(true);
                let haystack = format!("{} {} {} {}", track.title, track.artist, track.album, track.genre)
                    .to_ascii_lowercase();
                matches_folder && (search.is_empty() || haystack.contains(&search))
            })
            .cloned()
            .collect();
    }

    fn play_track(&mut self, track_id: i64) {
        let Some((path, metadata)) = self.all_tracks.iter().find(|track| track.id == track_id).map(|track| {
            (
                track.path.clone(),
                TrackMetadata {
                    title: track.title.clone(),
                    artist: track.artist.clone(),
                    album: track.album.clone(),
                    duration: track.duration,
                },
            )
        }) else {
            return;
        };

        self.playback.metadata = metadata;
        self.playback.elapsed = Duration::ZERO;
        self.selected_track = Some(track_id);
        self.current_playing_track_id = Some(track_id);
        self.is_playing = true;
        self.mpris_state.set_metadata(self.playback.metadata.clone());
        self.mpris_state.set_playing(true);
        self.playback_sender.publish_track(self.playback.metadata.clone());

        if self.audio_output.is_none() {
            if let Ok(output) = AudioOutput::open_default(self.playback_sender.clone()) {
                output.set_volume(self.volume);
                self.audio_output = Some(output);
            }
        }

        if let Some(audio_output) = &self.audio_output {
            let queue = audio_output.queue.clone();
            queue.clear();
            audio_output.reset_position();
            if let Err(error) = audio_output.play() {
                eprintln!("unable to start audio output: {error}");
                self.is_playing = false;
                self.mpris_state.set_playing(false);
                return;
            }

            let generation = self.playback_generation.fetch_add(1, Ordering::Relaxed) + 1;
            let playback_generation = Arc::clone(&self.playback_generation);
            let target_rate = audio_output.config.sample_rate.0;
            let target_channels = audio_output.config.channels;

            // Pre-decode and resample into device format for playback and seamless seeking
            match decode_track(&path) {
                Ok(track) => {
                    let resampled = resample_and_remap_channels(
                        &track.samples,
                        track.sample_rate,
                        track.channels,
                        target_rate,
                        target_channels,
                    );
                    let samples_arc = Arc::new(resampled);
                    self.current_track_samples = Some(Arc::clone(&samples_arc));
                    let samples_to_push = samples_arc.as_ref().clone();
                    thread::spawn(move || {
                        queue.push_interleaved_cancellable(samples_to_push, || {
                            playback_generation.load(Ordering::Relaxed) == generation
                        });
                    });
                }
                Err(error) => eprintln!("unable to decode {}: {error}", path.display()),
            }
        }
    }

    fn seek_to(&mut self, fraction: f32) {
        let Some(duration) = self.playback.metadata.duration else { return };
        if duration.is_zero() { return; }

        let target_secs = duration.as_secs_f64() * (fraction.clamp(0.0, 1.0) as f64);
        let target_duration = Duration::from_secs_f64(target_secs);
        self.playback.elapsed = target_duration;

        if let (Some(audio_output), Some(samples)) = (&self.audio_output, &self.current_track_samples) {
            audio_output.queue.clear();
            audio_output.set_position(target_duration);

            let sample_rate = audio_output.config.sample_rate.0 as usize;
            let channels = usize::from(audio_output.config.channels).max(1);
            let raw_offset = (target_secs * sample_rate as f64 * channels as f64) as usize;
            let sample_offset = (raw_offset.min(samples.len())) / channels * channels;
            let remaining_samples = samples[sample_offset..].to_vec();

            let queue = audio_output.queue.clone();
            let generation = self.playback_generation.fetch_add(1, Ordering::Relaxed) + 1;
            let playback_generation = Arc::clone(&self.playback_generation);
            thread::spawn(move || {
                queue.push_interleaved_cancellable(remaining_samples, || {
                    playback_generation.load(Ordering::Relaxed) == generation
                });
            });
        }
    }

    fn play_next(&mut self) {
        if self.is_repeated {
            if let Some(track_id) = self.current_playing_track_id {
                self.play_track(track_id);
                return;
            }
        }

        // 1. Pop from queue if items exist
        if let Some(track_id) = self.queued_track_ids.first().copied() {
            self.queued_track_ids.remove(0);
            self.play_track(track_id);
            return;
        }

        // 2. Play next in playlist
        let track_list = if !self.visible_tracks.is_empty() {
            &self.visible_tracks
        } else {
            &self.all_tracks
        };

        if track_list.is_empty() {
            return;
        }

        if self.is_shuffled {
            use std::time::SystemTime;
            let seed = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos() as usize;
            let random_idx = seed % track_list.len();
            self.play_track(track_list[random_idx].id);
            return;
        }

        if let Some(current_id) = self.current_playing_track_id {
            if let Some(curr_idx) = track_list.iter().position(|t| t.id == current_id) {
                let next_idx = (curr_idx + 1) % track_list.len();
                self.play_track(track_list[next_idx].id);
                return;
            }
        }

        if let Some(first) = track_list.first() {
            self.play_track(first.id);
        }
    }

    fn play_previous(&mut self) {
        if self.playback.elapsed > Duration::from_secs(3) {
            if let Some(id) = self.current_playing_track_id {
                self.play_track(id);
                return;
            }
        }

        let track_list = if !self.visible_tracks.is_empty() {
            &self.visible_tracks
        } else {
            &self.all_tracks
        };

        if track_list.is_empty() {
            return;
        }

        if let Some(current_id) = self.current_playing_track_id {
            if let Some(curr_idx) = track_list.iter().position(|t| t.id == current_id) {
                let prev_idx = if curr_idx == 0 {
                    track_list.len() - 1
                } else {
                    curr_idx - 1
                };
                self.play_track(track_list[prev_idx].id);
                return;
            }
        }

        if let Some(first) = track_list.first() {
            self.play_track(first.id);
        }
    }

    fn toggle_playback(&mut self) {
        if self.is_playing {
            self.pause_playback();
        } else if self.current_playing_track_id.is_some() {
            self.resume_playback();
        } else if let Some(first) = self.visible_tracks.first().or_else(|| self.all_tracks.first()) {
            self.play_track(first.id);
        }
    }

    fn resume_playback(&mut self) {
        if let Some(audio_output) = &self.audio_output {
            if let Err(error) = audio_output.play() {
                eprintln!("unable to resume audio output: {error}");
                return;
            }
        }
        self.is_playing = true;
        self.mpris_state.set_playing(true);
    }

    fn pause_playback(&mut self) {
        if let Some(audio_output) = &self.audio_output {
            if let Err(error) = audio_output.pause() {
                eprintln!("unable to pause audio output: {error}");
                return;
            }
        }
        self.is_playing = false;
        self.mpris_state.set_playing(false);
    }

    fn stop_playback(&mut self) {
        self.playback_generation.fetch_add(1, Ordering::Relaxed);
        if let Some(audio_output) = &self.audio_output {
            audio_output.queue.clear();
            audio_output.reset_position();
            if let Err(error) = audio_output.pause() {
                eprintln!("unable to stop audio output: {error}");
            }
        }
        self.is_playing = false;
        self.playback.elapsed = Duration::ZERO;
        self.mpris_state.set_playing(false);
    }
}

/// Generates pleasant synthetic demo WAV audio tracks and indexes them into the library.
async fn create_and_index_demo_tracks() -> Result<Vec<LibraryTrack>, String> {
    tokio::task::spawn_blocking(|| -> Result<Vec<LibraryTrack>, String> {
        let music_dir = std::env::temp_dir().join("kanono_demo_music");
        fs::create_dir_all(&music_dir).map_err(|e| e.to_string())?;

        let sample_rate = 44100_u32;

        // Track 1: Kanono Melodic Groove (12 seconds)
        let track1_path = music_dir.join("01 - Kanono Groove.wav");
        if !track1_path.exists() {
            write_synth_wav(&track1_path, sample_rate, 12, |t| {
                // Chord progression in A minor
                let bass = (2.0 * std::f32::consts::PI * 110.0 * t).sin() * 0.35;
                let melody_freq = match (t * 2.0) as u32 % 4 {
                    0 => 440.0,
                    1 => 523.25,
                    2 => 659.25,
                    _ => 587.33,
                };
                let lead = (2.0 * std::f32::consts::PI * melody_freq * t).sin() * 0.25;
                let pad = (2.0 * std::f32::consts::PI * 220.0 * t).sin() * 0.15;
                (bass + lead + pad).clamp(-0.95, 0.95)
            })?;
        }

        // Track 2: Ambient Reverie (15 seconds)
        let track2_path = music_dir.join("02 - Ambient Reverie.wav");
        if !track2_path.exists() {
            write_synth_wav(&track2_path, sample_rate, 15, |t| {
                let warm1 = (2.0 * std::f32::consts::PI * 196.0 * t).sin() * 0.25;
                let warm2 = (2.0 * std::f32::consts::PI * 246.94 * t).sin() * 0.25;
                let warm3 = (2.0 * std::f32::consts::PI * 293.66 * t).sin() * 0.2;
                let shimmer = (2.0 * std::f32::consts::PI * 880.0 * t).sin() * 0.08 * (t * 0.5).sin().abs();
                (warm1 + warm2 + warm3 + shimmer).clamp(-0.95, 0.95)
            })?;
        }

        // Track 3: Cyberpunk Chiptune (10 seconds)
        let track3_path = music_dir.join("03 - Cyberpunk Chiptune.wav");
        if !track3_path.exists() {
            write_synth_wav(&track3_path, sample_rate, 10, |t| {
                let step = (t * 6.0) as u32 % 6;
                let f = match step {
                    0 => 329.63,
                    1 => 392.00,
                    2 => 493.88,
                    3 => 587.33,
                    4 => 493.88,
                    _ => 392.00,
                };
                // Square wave
                let sq: f32 = if (t * f).fract() < 0.5 { 0.2 } else { -0.2 };
                let bass_sq: f32 = if (t * (f / 2.0)).fract() < 0.5 { 0.2 } else { -0.2 };
                (sq + bass_sq).clamp(-0.95, 0.95)
            })?;
        }

        let database_path = library_database_path();
        let mut library = LibraryDatabase::open(database_path).map_err(|e| e.to_string())?;
        library.scan_directory(&music_dir).map_err(|e| e.to_string())?;
        library.query(&TrackQuery::default()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

fn write_synth_wav(
    path: &Path,
    sample_rate: u32,
    seconds: u32,
    gen: impl Fn(f32) -> f32,
) -> Result<(), String> {
    let mut file = File::create(path).map_err(|e| e.to_string())?;
    let channels = 2_u16;
    let bits_per_sample = 16_u16;
    let total_samples = sample_rate * seconds;
    let data_len = total_samples * u32::from(channels) * u32::from(bits_per_sample / 8);
    let riff_len = 36 + data_len;

    // RIFF header
    file.write_all(b"RIFF").map_err(|e| e.to_string())?;
    file.write_all(&riff_len.to_le_bytes()).map_err(|e| e.to_string())?;
    file.write_all(b"WAVE").map_err(|e| e.to_string())?;

    // fmt subchunk
    file.write_all(b"fmt ").map_err(|e| e.to_string())?;
    file.write_all(&16_u32.to_le_bytes()).map_err(|e| e.to_string())?; // Subchunk1Size (16 for PCM)
    file.write_all(&1_u16.to_le_bytes()).map_err(|e| e.to_string())?; // AudioFormat (1 for PCM)
    file.write_all(&channels.to_le_bytes()).map_err(|e| e.to_string())?;
    file.write_all(&sample_rate.to_le_bytes()).map_err(|e| e.to_string())?;
    let byte_rate = sample_rate * u32::from(channels) * u32::from(bits_per_sample / 8);
    file.write_all(&byte_rate.to_le_bytes()).map_err(|e| e.to_string())?;
    let block_align = channels * (bits_per_sample / 8);
    file.write_all(&block_align.to_le_bytes()).map_err(|e| e.to_string())?;
    file.write_all(&bits_per_sample.to_le_bytes()).map_err(|e| e.to_string())?;

    // data subchunk
    file.write_all(b"data").map_err(|e| e.to_string())?;
    file.write_all(&data_len.to_le_bytes()).map_err(|e| e.to_string())?;

    let mut buf = Vec::with_capacity(4096);
    for s in 0..total_samples {
        let t = s as f32 / sample_rate as f32;
        let v = gen(t);
        let sample_i16 = (v * 32767.0).clamp(-32768.0, 32767.0) as i16;
        let bytes = sample_i16.to_le_bytes();
        // stereo: L + R
        buf.extend_from_slice(&bytes);
        buf.extend_from_slice(&bytes);

        if buf.len() >= 4096 {
            file.write_all(&buf).map_err(|e| e.to_string())?;
            buf.clear();
        }
    }
    if !buf.is_empty() {
        file.write_all(&buf).map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanono_audio_engine::decode_track;

    #[test]
    fn test_nav_tab_default() {
        assert_eq!(NavTab::default(), NavTab::Library);
    }

    #[test]
    fn test_write_synth_wav_and_decode() {
        let temp_dir = std::env::temp_dir().join("kanono_test_synth");
        let _ = fs::create_dir_all(&temp_dir);
        let wav_path = temp_dir.join("test_synth.wav");

        let res = write_synth_wav(&wav_path, 44100, 1, |t| {
            (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5
        });
        assert!(res.is_ok(), "WAV generation succeeded");

        let decoded = decode_track(&wav_path).expect("Decoded generated WAV");
        assert_eq!(decoded.sample_rate, 44100);
        assert_eq!(decoded.channels, 2);
        assert!(!decoded.samples.is_empty());
        assert!(decoded.metadata.duration.is_some());

        let _ = fs::remove_file(&wav_path);
        let _ = fs::remove_dir(&temp_dir);
    }
}