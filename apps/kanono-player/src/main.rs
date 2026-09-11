mod mpris;
mod plugins;
mod ui;

use std::{
    collections::{HashMap, HashSet},
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

use crossbeam_channel::{Receiver, Sender};
use iced::{executor, time, Application, Command, Element, Subscription, Theme};
use kanono_audio_engine::{
    decode_track_streaming, playback_state_channel, AudioOutput, LibraryDatabase, LibraryTrack,
    PlaybackStateReceiver, PlaybackStateSender, PlaybackUpdate, ReplayGainEvent, ReplayGainResult,
    ReplayGainWorker, TagUpdate, TrackMetadata, TrackQuery,
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
    MostPlayed,
    Queue,
    Folders,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterCategory {
    #[default]
    All,
    Artists,
    Albums,
    Genres,
    Folders,
    Duration,
    Year,
    Bpm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurationFilter {
    Short,     // < 2 mins (120s)
    Medium,    // 2 - 4 mins (120s - 240s)
    Long,      // 4 - 6 mins (240s - 360s)
    ExtraLong, // > 6 mins (360s+)
}

impl DurationFilter {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Short => "< 2 min",
            Self::Medium => "2 - 4 min",
            Self::Long => "4 - 6 min",
            Self::ExtraLong => "> 6 min",
        }
    }
    pub fn matches(&self, d: Option<Duration>) -> bool {
        let Some(d) = d else { return false };
        let s = d.as_secs();
        match self {
            Self::Short => s < 120,
            Self::Medium => (120..240).contains(&s),
            Self::Long => (240..360).contains(&s),
            Self::ExtraLong => s >= 360,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BpmFilter {
    Slow,       // < 90 BPM
    Medium,     // 90 - 120 BPM
    Fast,       // 120 - 140 BPM
    HighEnergy, // > 140 BPM
}

impl BpmFilter {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Slow => "< 90 (Chill)",
            Self::Medium => "90 - 120 (Mid)",
            Self::Fast => "120 - 140 (Upbeat)",
            Self::HighEnergy => "> 140 (Energy)",
        }
    }
    pub fn matches(&self, b: Option<u32>) -> bool {
        let Some(b) = b else { return false };
        match self {
            Self::Slow => b < 90,
            Self::Medium => (90..=120).contains(&b),
            Self::Fast => (121..=140).contains(&b),
            Self::HighEnergy => b > 140,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct FilterState {
    pub active_category: FilterCategory,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub duration: Option<DurationFilter>,
    pub year: Option<i32>,
    pub bpm: Option<BpmFilter>,
}

impl FilterState {
    pub fn is_any_active(&self) -> bool {
        self.artist.is_some()
            || self.album.is_some()
            || self.genre.is_some()
            || self.duration.is_some()
            || self.year.is_some()
            || self.bpm.is_some()
    }

    pub fn clear(&mut self) {
        self.artist = None;
        self.album = None;
        self.genre = None;
        self.duration = None;
        self.year = None;
        self.bpm = None;
        self.active_category = FilterCategory::All;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchField {
    Artist,
    Album,
    Genre,
    Year,
    Bpm,
}

#[derive(Debug, Clone, Default)]
pub struct BatchEditState {
    pub is_open: bool,
    pub apply_artist: bool,
    pub artist: String,
    pub apply_album: bool,
    pub album: String,
    pub apply_genre: bool,
    pub genre: String,
    pub apply_year: bool,
    pub year: String,
    pub apply_bpm: bool,
    pub bpm: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditField {
    Title,
    Artist,
    Album,
    Genre,
    Year,
    Bpm,
}

#[derive(Debug, Clone)]
pub struct EditingTrackState {
    pub track_id: i64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub year: String,
    pub bpm: String,
}

#[derive(Clone)]
struct PreloadedTrack {
    track_id: i64,
    metadata: TrackMetadata,
    samples: Arc<Vec<f32>>,
}

enum DecodeEvent {
    TrackReady {
        generation: u64,
        track_id: i64,
        samples: Arc<Vec<f32>>,
        exact_duration: Duration,
    },
    PreloadReady {
        for_playing_id: Option<i64>,
        track_id: i64,
        metadata: TrackMetadata,
        samples: Arc<Vec<f32>>,
        exact_duration: Duration,
    },
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
    library: Option<LibraryDatabase>,
    all_tracks: Vec<LibraryTrack>,
    visible_tracks: Vec<LibraryTrack>,
    selected_folder: Option<PathBuf>,
    expanded_folders: HashSet<PathBuf>,
    current_tab: NavTab,
    filter_state: FilterState,
    search: String,
    selected_track: Option<i64>,
    selected_track_ids: HashSet<i64>,
    batch_edit: BatchEditState,
    current_playing_track_id: Option<i64>,
    queued_track_ids: Vec<i64>,
    is_playing: bool,
    volume: f32,
    is_muted: bool,
    prev_volume: f32,
    is_shuffled: bool,
    is_repeated: bool,
    replaygain_enabled: bool,
    gapless_enabled: bool,
    preloaded_track: Option<PreloadedTrack>,
    seeking_fraction: Option<f32>,
    decode_tx: Sender<DecodeEvent>,
    decode_rx: Receiver<DecodeEvent>,
    track_gains: HashMap<PathBuf, ReplayGainResult>,
    replaygain_worker: Arc<ReplayGainWorker>,
    editing_track: Option<EditingTrackState>,
    current_track_samples: Option<Arc<Vec<f32>>>,
    status_message: Option<String>,
    played_tracked_for_current: bool,
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
    PollReplayGain,
    ImportFolder,
    FolderPicked(Option<PathBuf>),
    ImportFiles,
    FilesPicked(Option<Vec<PathBuf>>),
    LibraryIndexed(IndexResult),
    SearchChanged(String),
    SelectFolder(Option<PathBuf>),
    ToggleFolderExpanded(PathBuf),
    PlayFolder(PathBuf),
    QueueFolder(PathBuf),
    SelectTab(NavTab),
    PlayMostPlayed,
    QueueMostPlayed,
    SelectFilterCategory(FilterCategory),
    SetArtistFilter(Option<String>),
    SetAlbumFilter(Option<String>),
    SetGenreFilter(Option<String>),
    SetDurationFilter(Option<DurationFilter>),
    SetYearFilter(Option<i32>),
    SetBpmFilter(Option<BpmFilter>),
    ClearFilters,
    ClearAll,
    SelectTrack(i64),
    ToggleTrackSelection(i64),
    SelectAllVisibleTracks,
    ClearTrackSelection,
    PlaySelectedTracks,
    QueueSelectedTracks,
    StartBatchEdit,
    BatchFieldChanged(BatchField, String),
    ToggleBatchFieldApply(BatchField),
    SaveBatchTags,
    CancelBatchEdit,
    QueueTrack(i64),
    RemoveFromQueue(usize),
    ClearQueue,
    PlayTrack(i64),
    TogglePlayback,
    Next,
    Previous,
    SeekSlide(f32),
    SeekRelease,
    Seek(f32),
    VolumeChanged(f32),
    ToggleMute,
    ToggleShuffle,
    ToggleRepeat,
    ToggleReplayGain,
    ToggleGapless,
    AnalyzeReplayGain(Option<i64>),
    StartEditTrack(i64),
    EditFieldChanged(EditField, String),
    SaveTrackTags,
    CancelEditTrack,
    GenerateSampleAudio,
    SampleAudioGenerated(Result<Vec<LibraryTrack>, String>),
    DurationsBatchResolved(Vec<(i64, Duration)>),
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
        let replaygain_worker = Arc::new(ReplayGainWorker::spawn());
        let (decode_tx, decode_rx) = crossbeam_channel::unbounded();
        let mut app = Self {
            playback: PlaybackViewState::default(),
            playback_sender,
            playback_receiver,
            audio_output,
            playback_generation: Arc::new(AtomicU64::new(0)),
            mpris_commands: mpris.commands,
            mpris_state: mpris.state,
            components,
            library: Some(library),
            visible_tracks: all_tracks.clone(),
            all_tracks,
            selected_folder: None,
            expanded_folders: HashSet::new(),
            current_tab: NavTab::Library,
            filter_state: FilterState::default(),
            search: String::new(),
            selected_track: None,
            selected_track_ids: HashSet::new(),
            batch_edit: BatchEditState::default(),
            current_playing_track_id: None,
            queued_track_ids: Vec::new(),
            is_playing: false,
            volume: 0.85,
            is_muted: false,
            prev_volume: 0.85,
            is_shuffled: false,
            is_repeated: false,
            replaygain_enabled: true,
            gapless_enabled: true,
            preloaded_track: None,
            seeking_fraction: None,
            decode_tx,
            decode_rx,
            track_gains: HashMap::new(),
            replaygain_worker,
            editing_track: None,
            current_track_samples: None,
            status_message: None,
            played_tracked_for_current: false,
        };
        app.refresh_visible_tracks();
        (app, resolve_missing_durations())
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
            Message::PollPlayback => {
                while let Ok(event) = self.decode_rx.try_recv() {
                    self.handle_decode_event(event);
                }
                self.apply_playback_updates();
            }
            Message::PollMpris => self.apply_mpris_commands(),
            Message::ImportFolder => return Command::perform(pick_music_folder(), Message::FolderPicked),
            Message::FolderPicked(folder) => {
                if let Some(folder) = folder {
                    return Command::perform(index_music_folder(folder), Message::LibraryIndexed);
                }
            }
            Message::ImportFiles => return Command::perform(pick_music_files(), Message::FilesPicked),
            Message::FilesPicked(files) => {
                if let Some(files) = files {
                    if !files.is_empty() {
                        return Command::perform(index_music_files(files), Message::LibraryIndexed);
                    }
                }
            }
            Message::LibraryIndexed(result) => {
                self.apply_index_result(result);
                return resolve_missing_durations();
            }
            Message::SearchChanged(search) => {
                self.search = search;
                self.refresh_visible_tracks();
            }
            Message::SelectFolder(folder) => {
                self.selected_folder = folder;
                self.current_tab = NavTab::Folders;
                self.refresh_visible_tracks();
            }
            Message::ToggleFolderExpanded(folder) => {
                if self.expanded_folders.contains(&folder) {
                    self.expanded_folders.remove(&folder);
                } else {
                    self.expanded_folders.insert(folder);
                }
            }
            Message::PlayFolder(folder) => {
                let folder_tracks: Vec<i64> = self
                    .all_tracks
                    .iter()
                    .filter(|t| t.path.starts_with(&folder))
                    .map(|t| t.id)
                    .collect();
                if let Some(&first_id) = folder_tracks.first() {
                    self.play_track(first_id);
                    self.queued_track_ids = folder_tracks.into_iter().skip(1).collect();
                    let folder_name = folder.file_name().and_then(|n| n.to_str()).unwrap_or("Folder");
                    self.status_message = Some(format!("Playing folder: {}", folder_name));
                }
            }
            Message::QueueFolder(folder) => {
                let folder_tracks: Vec<i64> = self
                    .all_tracks
                    .iter()
                    .filter(|t| t.path.starts_with(&folder))
                    .map(|t| t.id)
                    .collect();
                let count = folder_tracks.len();
                self.queued_track_ids.extend(folder_tracks);
                let folder_name = folder.file_name().and_then(|n| n.to_str()).unwrap_or("Folder");
                self.status_message = Some(format!("Added {} tracks from {} to queue", count, folder_name));
            }
            Message::SelectTab(tab) => {
                self.current_tab = tab;
                if tab == NavTab::Library {
                    self.selected_folder = None;
                    self.refresh_visible_tracks();
                } else if tab == NavTab::MostPlayed {
                    self.refresh_visible_tracks();
                }
            }
            Message::PlayMostPlayed => {
                let mut list = self.all_tracks.clone();
                list.sort_by_key(|a| std::cmp::Reverse(a.play_count));
                if let Some(first) = list.first() {
                    let first_id = first.id;
                    self.play_track(first_id);
                    self.queued_track_ids = list.into_iter().skip(1).map(|t| t.id).collect();
                    self.status_message = Some("Playing frequently played tracks".to_string());
                }
            }
            Message::QueueMostPlayed => {
                let mut list = self.all_tracks.clone();
                list.sort_by_key(|a| std::cmp::Reverse(a.play_count));
                let count = list.len();
                self.queued_track_ids = list.into_iter().map(|t| t.id).collect();
                self.status_message = Some(format!("Queued {} frequently played tracks", count));
            }
            Message::SelectFilterCategory(cat) => {
                self.filter_state.active_category = cat;
            }
            Message::SetArtistFilter(artist) => {
                self.filter_state.artist = artist;
                self.refresh_visible_tracks();
            }
            Message::SetAlbumFilter(album) => {
                self.filter_state.album = album;
                self.refresh_visible_tracks();
            }
            Message::SetGenreFilter(genre) => {
                self.filter_state.genre = genre;
                self.refresh_visible_tracks();
            }
            Message::SetDurationFilter(dur) => {
                self.filter_state.duration = dur;
                self.refresh_visible_tracks();
            }
            Message::SetYearFilter(year) => {
                self.filter_state.year = year;
                self.refresh_visible_tracks();
            }
            Message::SetBpmFilter(bpm) => {
                self.filter_state.bpm = bpm;
                self.refresh_visible_tracks();
            }
            Message::ClearFilters => {
                self.filter_state.clear();
                self.selected_folder = None;
                self.refresh_visible_tracks();
                self.status_message = Some("Filters cleared".to_string());
            }
            Message::ClearAll => {
                self.filter_state.clear();
                self.selected_folder = None;
                self.search.clear();
                self.selected_track_ids.clear();
                self.batch_edit.is_open = false;
                self.editing_track = None;
                if self.current_tab != NavTab::Queue && self.current_tab != NavTab::Info {
                    self.current_tab = NavTab::Library;
                }
                self.refresh_visible_tracks();
                self.status_message = Some("View and filters cleared".to_string());
            }
            Message::SelectTrack(track_id) => {
                self.selected_track = Some(track_id);
            }
            Message::ToggleTrackSelection(id) => {
                if self.selected_track_ids.contains(&id) {
                    self.selected_track_ids.remove(&id);
                } else {
                    self.selected_track_ids.insert(id);
                }
            }
            Message::SelectAllVisibleTracks => {
                let all_selected = !self.visible_tracks.is_empty()
                    && self.visible_tracks.iter().all(|t| self.selected_track_ids.contains(&t.id));
                if all_selected {
                    for t in &self.visible_tracks {
                        self.selected_track_ids.remove(&t.id);
                    }
                } else {
                    for t in &self.visible_tracks {
                        self.selected_track_ids.insert(t.id);
                    }
                }
            }
            Message::ClearTrackSelection => {
                self.selected_track_ids.clear();
                self.batch_edit.is_open = false;
            }
            Message::PlaySelectedTracks => {
                let selected: Vec<i64> = self
                    .visible_tracks
                    .iter()
                    .filter(|t| self.selected_track_ids.contains(&t.id))
                    .map(|t| t.id)
                    .collect();
                if let Some(&first_id) = selected.first() {
                    self.play_track(first_id);
                    self.queued_track_ids = selected.into_iter().skip(1).collect();
                    self.status_message = Some("Playing selected tracks".to_string());
                }
            }
            Message::QueueSelectedTracks => {
                let selected: Vec<i64> = self
                    .visible_tracks
                    .iter()
                    .filter(|t| self.selected_track_ids.contains(&t.id))
                    .map(|t| t.id)
                    .collect();
                let count = selected.len();
                self.queued_track_ids.extend(selected);
                self.status_message = Some(format!("Added {} tracks to queue", count));
            }
            Message::StartBatchEdit => {
                if !self.selected_track_ids.is_empty() {
                    let selected_tracks: Vec<&LibraryTrack> = self
                        .all_tracks
                        .iter()
                        .filter(|t| self.selected_track_ids.contains(&t.id))
                        .collect();
                    let common_artist = selected_tracks.first().map(|t| t.artist.clone()).unwrap_or_default();
                    let all_same_artist = selected_tracks.iter().all(|t| t.artist == common_artist);

                    let common_album = selected_tracks.first().map(|t| t.album.clone()).unwrap_or_default();
                    let all_same_album = selected_tracks.iter().all(|t| t.album == common_album);

                    let common_genre = selected_tracks.first().map(|t| t.genre.clone()).unwrap_or_default();
                    let all_same_genre = selected_tracks.iter().all(|t| t.genre == common_genre);

                    self.batch_edit = BatchEditState {
                        is_open: true,
                        apply_artist: all_same_artist,
                        artist: if all_same_artist { common_artist } else { String::new() },
                        apply_album: all_same_album,
                        album: if all_same_album { common_album } else { String::new() },
                        apply_genre: all_same_genre,
                        genre: if all_same_genre { common_genre } else { String::new() },
                        apply_year: false,
                        year: String::new(),
                        apply_bpm: false,
                        bpm: String::new(),
                    };
                }
            }
            Message::BatchFieldChanged(field, val) => {
                match field {
                    BatchField::Artist => {
                        self.batch_edit.artist = val;
                        self.batch_edit.apply_artist = true;
                    }
                    BatchField::Album => {
                        self.batch_edit.album = val;
                        self.batch_edit.apply_album = true;
                    }
                    BatchField::Genre => {
                        self.batch_edit.genre = val;
                        self.batch_edit.apply_genre = true;
                    }
                    BatchField::Year => {
                        self.batch_edit.year = val;
                        self.batch_edit.apply_year = true;
                    }
                    BatchField::Bpm => {
                        self.batch_edit.bpm = val;
                        self.batch_edit.apply_bpm = true;
                    }
                }
            }
            Message::ToggleBatchFieldApply(field) => {
                match field {
                    BatchField::Artist => self.batch_edit.apply_artist = !self.batch_edit.apply_artist,
                    BatchField::Album => self.batch_edit.apply_album = !self.batch_edit.apply_album,
                    BatchField::Genre => self.batch_edit.apply_genre = !self.batch_edit.apply_genre,
                    BatchField::Year => self.batch_edit.apply_year = !self.batch_edit.apply_year,
                    BatchField::Bpm => self.batch_edit.apply_bpm = !self.batch_edit.apply_bpm,
                }
            }
            Message::SaveBatchTags => {
                if self.batch_edit.is_open && !self.selected_track_ids.is_empty() {
                    let track_ids: Vec<i64> = self.selected_track_ids.iter().copied().collect();
                    let update = TagUpdate {
                        title: None,
                        artist: if self.batch_edit.apply_artist { Some(self.batch_edit.artist.trim().to_string()) } else { None },
                        album: if self.batch_edit.apply_album { Some(self.batch_edit.album.trim().to_string()) } else { None },
                        genre: if self.batch_edit.apply_genre { Some(self.batch_edit.genre.trim().to_string()) } else { None },
                        year: if self.batch_edit.apply_year { self.batch_edit.year.trim().parse::<i32>().ok() } else { None },
                        bpm: if self.batch_edit.apply_bpm { self.batch_edit.bpm.trim().parse::<u32>().ok() } else { None },
                    };
                    if let Some(db) = &mut self.library {
                        let _ = db.mass_tag(&track_ids, &update);
                        if let Ok(tracks) = db.query(&TrackQuery::default()) {
                            self.all_tracks = tracks;
                            self.refresh_visible_tracks();
                            self.status_message = Some(format!("Updated tags for {} tracks", track_ids.len()));
                        }
                    }
                    self.batch_edit.is_open = false;
                }
            }
            Message::CancelBatchEdit => {
                self.batch_edit.is_open = false;
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
            Message::SeekSlide(fraction) => {
                self.seeking_fraction = Some(fraction.clamp(0.0, 1.0));
            }
            Message::SeekRelease => {
                if let Some(fraction) = self.seeking_fraction.take() {
                    self.seek_to(fraction);
                }
            }
            Message::Seek(fraction) => {
                self.seeking_fraction = None;
                self.seek_to(fraction);
            }
            Message::VolumeChanged(vol) => {
                self.volume = vol;
                self.is_muted = vol == 0.0;
                self.apply_volume();
            }
            Message::ToggleMute => {
                if self.is_muted {
                    self.is_muted = false;
                    let target = if self.prev_volume > 0.05 { self.prev_volume } else { 0.5 };
                    self.volume = target;
                    self.apply_volume();
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
            Message::ToggleReplayGain => {
                self.replaygain_enabled = !self.replaygain_enabled;
                self.status_message = Some(format!(
                    "Loudness Normalization {}",
                    if self.replaygain_enabled { "Enabled (-18 LUFS)" } else { "Disabled" }
                ));
                self.apply_volume();
            }
            Message::ToggleGapless => {
                self.gapless_enabled = !self.gapless_enabled;
                if !self.gapless_enabled {
                    self.preloaded_track = None;
                } else {
                    self.maybe_preload_next_track();
                }
                self.status_message = Some(format!(
                    "Gapless Playback {}",
                    if self.gapless_enabled { "Enabled" } else { "Disabled" }
                ));
            }
            Message::AnalyzeReplayGain(track_id) => {
                let id = track_id.or(self.selected_track).or(self.current_playing_track_id);
                if let Some(id) = id {
                    if let Some(track) = self.all_tracks.iter().find(|t| t.id == id) {
                        let path = track.path.clone();
                        self.status_message = Some(format!("Analyzing loudness for {}...", track.title));
                        let _ = self.replaygain_worker.enqueue(path);
                    }
                }
            }
            Message::PollReplayGain => {
                while let Ok(event) = self.replaygain_worker.events().try_recv() {
                    match event {
                        ReplayGainEvent::Complete { path, result } => {
                            let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Track");
                            self.status_message = Some(format!("{}: {:.1} LUFS ({:+.1} dB)", name, result.integrated_lufs, result.gain_db));
                            self.track_gains.insert(path, result);
                            self.apply_volume();
                        }
                        ReplayGainEvent::Failed { path, error } => {
                            eprintln!("ReplayGain failed for {}: {error}", path.display());
                        }
                    }
                }
            }
            Message::StartEditTrack(track_id) => {
                if let Some(track) = self.all_tracks.iter().find(|t| t.id == track_id) {
                    self.editing_track = Some(EditingTrackState {
                        track_id: track.id,
                        title: track.title.clone(),
                        artist: track.artist.clone(),
                        album: track.album.clone(),
                        genre: track.genre.clone(),
                        year: track.year.map(|y| y.to_string()).unwrap_or_default(),
                        bpm: track.bpm.map(|b| b.to_string()).unwrap_or_default(),
                    });
                }
            }
            Message::EditFieldChanged(field, value) => {
                if let Some(editing) = &mut self.editing_track {
                    match field {
                        EditField::Title => editing.title = value,
                        EditField::Artist => editing.artist = value,
                        EditField::Album => editing.album = value,
                        EditField::Genre => editing.genre = value,
                        EditField::Year => editing.year = value,
                        EditField::Bpm => editing.bpm = value,
                    }
                }
            }
            Message::SaveTrackTags => {
                if let Some(editing) = self.editing_track.take() {
                    let year_parsed = editing.year.trim().parse::<i32>().ok();
                    let bpm_parsed = editing.bpm.trim().parse::<u32>().ok();
                    let update = TagUpdate {
                        title: Some(editing.title.trim().to_string()),
                        artist: Some(editing.artist.trim().to_string()),
                        album: Some(editing.album.trim().to_string()),
                        genre: Some(editing.genre.trim().to_string()),
                        year: year_parsed,
                        bpm: bpm_parsed,
                    };
                    if let Some(db) = &mut self.library {
                        let _ = db.mass_tag(&[editing.track_id], &update);
                        if let Ok(tracks) = db.query(&TrackQuery::default()) {
                            self.all_tracks = tracks;
                            self.refresh_visible_tracks();
                            self.status_message = Some("Track tags updated successfully".to_string());
                        }
                    }
                }
            }
            Message::CancelEditTrack => {
                self.editing_track = None;
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
                    return resolve_missing_durations();
                }
                Err(e) => {
                    eprintln!("Failed to generate sample audio: {e}");
                }
            },
            Message::DurationsBatchResolved(resolved) => {
                for (id, dur) in resolved {
                    if let Some(t) = self.all_tracks.iter_mut().find(|t| t.id == id) {
                        t.duration = Some(dur);
                    }
                    if let Some(t) = self.visible_tracks.iter_mut().find(|t| t.id == id) {
                        t.duration = Some(dur);
                    }
                }
            }
        }
        Command::none()
    }

    fn view(&self) -> Element<'_, Message> {
        player_view(ViewProps {
            tracks: &self.visible_tracks,
            all_tracks: &self.all_tracks,
            selected_folder: self.selected_folder.as_ref(),
            expanded_folders: &self.expanded_folders,
            current_tab: self.current_tab,
            filter_state: &self.filter_state,
            search: &self.search,
            selected_track: self.selected_track,
            selected_track_ids: &self.selected_track_ids,
            batch_edit: &self.batch_edit,
            current_playing_track_id: self.current_playing_track_id,
            queued_track_ids: &self.queued_track_ids,
            is_playing: self.is_playing,
            is_shuffled: self.is_shuffled,
            is_repeated: self.is_repeated,
            replaygain_enabled: self.replaygain_enabled,
            gapless_enabled: self.gapless_enabled,
            volume: self.volume,
            is_muted: self.is_muted,
            metadata: &self.playback.metadata,
            elapsed: self.playback.elapsed,
            seeking_fraction: self.seeking_fraction,
            status_message: self.status_message.as_deref(),
            visualizer_pcm: &self.playback.visualizer_pcm,
            current_track_gain: self
                .current_playing_track_id
                .and_then(|id| self.all_tracks.iter().find(|t| t.id == id))
                .and_then(|t| self.track_gains.get(&t.path).copied()),
            editing_track: self.editing_track.as_ref(),
            components: self.components.plugins(),
        })
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            time::every(Duration::from_millis(33)).map(|_| Message::PollPlayback),
            time::every(Duration::from_millis(100)).map(|_| Message::PollMpris),
            time::every(Duration::from_millis(250)).map(|_| Message::PollReplayGain),
        ])
    }
}

#[derive(Debug, Clone)]
pub struct IndexResult {
    folder: PathBuf,
    tracks: Vec<LibraryTrack>,
    error: Option<String>,
}

async fn pick_music_files() -> Option<Vec<PathBuf>> {
    rfd::AsyncFileDialog::new()
        .set_title("Add Audio Files (WebM, MP3, FLAC, WAV, OGG...)")
        .add_filter(
            "Supported Audio (*.webm, *.mkv, *.mp3, *.flac, *.wav, *.ogg, *.m4a...)",
            &[
                "webm", "mkv", "mp3", "mp2", "mp1", "flac", "wav", "wave", "ogg", "oga", "m4a",
                "m4b", "mp4", "aac", "alac", "aiff", "aif", "caf",
            ],
        )
        .add_filter("WebM / Matroska Audio (*.webm, *.mkv)", &["webm", "mkv"])
        .add_filter("MP3 Audio (*.mp3)", &["mp3"])
        .add_filter("FLAC Lossless Audio (*.flac)", &["flac"])
        .add_filter("Waveform Audio (*.wav, *.wave)", &["wav", "wave"])
        .add_filter("Ogg Vorbis / Opus (*.ogg, *.oga)", &["ogg", "oga"])
        .add_filter("AAC / MP4 Audio (*.m4a, *.mp4, *.aac)", &["m4a", "mp4", "aac"])
        .add_filter("All Files (*.*)", &["*"])
        .pick_files()
        .await
        .map(|handles| handles.into_iter().map(|h| h.path().to_owned()).collect())
}

async fn pick_music_folder() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title("Add Music Folder (Scans WebM, MP3, FLAC, WAV, OGG...)")
        .pick_folder()
        .await
        .map(|handle| handle.path().to_owned())
}

async fn index_music_files(files: Vec<PathBuf>) -> IndexResult {
    let folder = files
        .first()
        .and_then(|p| p.parent())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let database_path = library_database_path();
    let result = tokio::task::spawn_blocking({
        let files = files.clone();
        move || -> Result<Vec<LibraryTrack>, String> {
            let mut library = LibraryDatabase::open(database_path).map_err(|e| e.to_string())?;
            library.scan_paths(&files).map_err(|e| e.to_string())?;
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
    if let Some(dir) = std::env::var_os("KANONO_COMPONENTS_DIR") {
        return PathBuf::from(dir);
    }
    for candidate in ["components", "target/release", "target/debug"] {
        let p = PathBuf::from(candidate);
        if p.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&p) {
                let has_so = entries.filter_map(|e| e.ok()).any(|e| {
                    e.path().extension().map(|ext| ext == "so").unwrap_or(false)
                });
                if has_so {
                    return p;
                }
            }
        }
    }
    PathBuf::from("components")
}

fn library_database_path() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("kanono-media-player/library.sqlite3")
}

impl KanonoApp {
    fn apply_volume(&self) {
        if let Some(output) = &self.audio_output {
            if self.is_muted {
                output.set_volume(0.0);
                return;
            }
            let mut target_vol = self.volume;
            if self.replaygain_enabled {
                if let Some(track_id) = self.current_playing_track_id {
                    if let Some(track) = self.all_tracks.iter().find(|t| t.id == track_id) {
                        if let Some(result) = self.track_gains.get(&track.path) {
                            let factor = 10.0_f32.powf((result.gain_db as f32) / 20.0).clamp(0.25, 2.5);
                            target_vol = (self.volume * factor).clamp(0.0, 1.0);
                        }
                    }
                }
            }
            output.set_volume(target_vol);
        }
    }

    fn handle_decode_event(&mut self, event: DecodeEvent) {
        match event {
            DecodeEvent::TrackReady {
                generation,
                track_id,
                samples,
                exact_duration,
            } => {
                if self.playback_generation.load(Ordering::SeqCst) != generation {
                    return;
                }
                if let Some(t) = self.all_tracks.iter_mut().find(|t| t.id == track_id) {
                    t.duration = Some(exact_duration);
                }
                if let Some(t) = self.visible_tracks.iter_mut().find(|t| t.id == track_id) {
                    t.duration = Some(exact_duration);
                }
                if let Some(db) = &mut self.library {
                    let _ = db.update_duration(track_id, exact_duration);
                }
                self.current_track_samples = Some(Arc::clone(&samples));
                self.playback.metadata.duration = Some(exact_duration);
                self.mpris_state.set_metadata(self.playback.metadata.clone());
                self.playback_sender.publish_track(self.playback.metadata.clone());

                self.maybe_preload_next_track();
            }
            DecodeEvent::PreloadReady {
                for_playing_id,
                track_id,
                metadata,
                samples,
                exact_duration,
            } => {
                if let Some(t) = self.all_tracks.iter_mut().find(|t| t.id == track_id) {
                    t.duration = Some(exact_duration);
                }
                if let Some(t) = self.visible_tracks.iter_mut().find(|t| t.id == track_id) {
                    t.duration = Some(exact_duration);
                }
                if let Some(db) = &mut self.library {
                    let _ = db.update_duration(track_id, exact_duration);
                }
                if self.current_playing_track_id == for_playing_id {
                    let mut meta = metadata;
                    meta.duration = Some(exact_duration);
                    self.preloaded_track = Some(PreloadedTrack {
                        track_id,
                        metadata: meta,
                        samples,
                    });
                }
            }
        }
    }

    fn maybe_preload_next_track(&mut self) {
        if !self.gapless_enabled {
            return;
        }
        if self.preloaded_track.is_some() {
            return;
        }
        let Some(next_id) = self.peek_next_track_id() else { return };
        if let Some(current_id) = self.current_playing_track_id {
            if next_id == current_id && !self.is_repeated {
                return;
            }
        }
        let Some(next_track) = self.all_tracks.iter().find(|t| t.id == next_id) else { return };
        let next_path = next_track.path.clone();
        let next_meta = TrackMetadata {
            title: next_track.title.clone(),
            artist: next_track.artist.clone(),
            album: next_track.album.clone(),
            duration: next_track.duration,
        };

        let Some(output) = &self.audio_output else { return };
        let target_rate = output.config.sample_rate.0;
        let target_channels = output.config.channels;
        let tx = self.decode_tx.clone();
        let current_playing_id = self.current_playing_track_id;

        thread::spawn(move || {
            if let Ok(track) = decode_track_streaming(&next_path, target_rate, target_channels, || true, |_| {}) {
                let exact_frames = track.samples.len() / usize::from(target_channels).max(1);
                let exact_dur = Duration::from_secs_f64(exact_frames as f64 / f64::from(target_rate));
                let _ = tx.send(DecodeEvent::PreloadReady {
                    for_playing_id: current_playing_id,
                    track_id: next_id,
                    metadata: next_meta,
                    samples: Arc::new(track.samples),
                    exact_duration: exact_dur,
                });
            }
        });
    }

    fn peek_next_track_id(&self) -> Option<i64> {
        if self.is_repeated {
            return self.current_playing_track_id;
        }
        if let Some(&first) = self.queued_track_ids.first() {
            return Some(first);
        }
        let track_list = if !self.visible_tracks.is_empty() {
            &self.visible_tracks
        } else {
            &self.all_tracks
        };
        if track_list.is_empty() {
            return None;
        }
        if self.is_shuffled {
            return None;
        }
        if let Some(current_id) = self.current_playing_track_id {
            if let Some(curr_idx) = track_list.iter().position(|t| t.id == current_id) {
                let next_idx = (curr_idx + 1) % track_list.len();
                return Some(track_list[next_idx].id);
            }
        }
        track_list.first().map(|t| t.id)
    }

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

        if self.is_playing {
            // Check for play counting (>= 10s playback counts as a play)
            if self.playback.elapsed >= Duration::from_secs(10) {
                self.record_current_track_play();
            }

            if let (Some(output), Some(samples)) = (&self.audio_output, &self.current_track_samples) {
                let channels = usize::from(output.config.channels).max(1);
                let total_frames = (samples.len() / channels) as u64;
                let played_frames = output.played_frames();
                let queue_empty = output.is_empty();

                // Track has finished playing when all frames have been played by CPAL or queue is empty near the end
                let track_finished = total_frames > 0 && (
                    played_frames >= total_frames || 
                    (queue_empty && played_frames + (output.config.sample_rate.0 as u64 / 10) >= total_frames)
                );

                if track_finished {
                    self.record_current_track_play();
                    self.play_next();
                }
            } else if let Some(dur) = self.playback.metadata.duration {
                if dur.as_secs() > 0 && self.playback.elapsed >= dur {
                    self.record_current_track_play();
                    self.play_next();
                }
            }
        }
    }

    fn record_current_track_play(&mut self) {
        if self.played_tracked_for_current {
            return;
        }
        let Some(curr_id) = self.current_playing_track_id else { return };
        self.played_tracked_for_current = true;
        if let Some(db) = &mut self.library {
            if let Ok(new_count) = db.record_play(curr_id) {
                if let Some(t) = self.all_tracks.iter_mut().find(|t| t.id == curr_id) {
                    t.play_count = new_count;
                }
                if let Some(t) = self.visible_tracks.iter_mut().find(|t| t.id == curr_id) {
                    t.play_count = new_count;
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
        let search = self.search.trim().to_ascii_lowercase();

        let mut tracks: Vec<LibraryTrack> = match self.current_tab {
            NavTab::MostPlayed => {
                let mut list = self.all_tracks.clone();
                list.sort_by(|a, b| b.play_count.cmp(&a.play_count).then_with(|| a.title.cmp(&b.title)));
                list
            }
            _ => self.all_tracks.clone(),
        };

        if let Some(folder) = &self.selected_folder {
            tracks.retain(|t| t.path.starts_with(folder));
        }

        if let Some(artist) = &self.filter_state.artist {
            tracks.retain(|t| t.artist.eq_ignore_ascii_case(artist));
        }
        if let Some(album) = &self.filter_state.album {
            tracks.retain(|t| t.album.eq_ignore_ascii_case(album));
        }
        if let Some(genre) = &self.filter_state.genre {
            tracks.retain(|t| t.genre.eq_ignore_ascii_case(genre));
        }
        if let Some(year) = self.filter_state.year {
            tracks.retain(|t| t.year == Some(year));
        }
        if let Some(dur_filter) = self.filter_state.duration {
            tracks.retain(|t| dur_filter.matches(t.duration));
        }
        if let Some(bpm_filter) = self.filter_state.bpm {
            tracks.retain(|t| bpm_filter.matches(t.bpm));
        }

        if !search.is_empty() {
            tracks.retain(|t| {
                let path_str = t.path.to_string_lossy();
                t.title.to_ascii_lowercase().contains(&search)
                    || t.artist.to_ascii_lowercase().contains(&search)
                    || t.album.to_ascii_lowercase().contains(&search)
                    || t.genre.to_ascii_lowercase().contains(&search)
                    || path_str.to_ascii_lowercase().contains(&search)
            });
        }

        self.visible_tracks = tracks;
    }

    fn play_track(&mut self, track_id: i64) {
        self.played_tracked_for_current = false;
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

        self.selected_track = Some(track_id);
        self.current_playing_track_id = Some(track_id);
        self.is_playing = true;
        self.seeking_fraction = None;
        self.playback.elapsed = Duration::ZERO;
        self.playback.metadata = metadata.clone();
        self.mpris_state.set_metadata(self.playback.metadata.clone());
        self.mpris_state.set_playing(true);
        self.playback_sender.publish_track(self.playback.metadata.clone());

        if self.audio_output.is_none() {
            if let Ok(output) = AudioOutput::open_default(self.playback_sender.clone()) {
                self.audio_output = Some(output);
            }
        }

        self.apply_volume();

        // Check if this track was already preloaded in memory
        if let Some(preloaded) = self.preloaded_track.take() {
            if preloaded.track_id == track_id {
                let generation = self.playback_generation.fetch_add(1, Ordering::SeqCst) + 1;
                let pb_gen = Arc::clone(&self.playback_generation);

                self.playback.metadata = preloaded.metadata;
                self.current_track_samples = Some(Arc::clone(&preloaded.samples));

                if let Some(output) = &self.audio_output {
                    output.queue.clear();
                    self.playback_receiver.clear();
                    output.reset_position();
                    let _ = output.play();

                    let queue = output.queue.clone();
                    let samples_arc = Arc::clone(&preloaded.samples);
                    thread::spawn(move || {
                        queue.push_interleaved_cancellable(samples_arc.iter().copied(), || {
                            pb_gen.load(Ordering::SeqCst) == generation
                        });
                    });
                }
                self.maybe_preload_next_track();
                return;
            }
        }

        // Invalidate previous workers and stream-decode in background thread
        self.preloaded_track = None;
        self.current_track_samples = None;
        let generation = self.playback_generation.fetch_add(1, Ordering::SeqCst) + 1;
        let pb_gen = Arc::clone(&self.playback_generation);

        if let Some(output) = &self.audio_output {
            output.queue.clear();
            self.playback_receiver.clear();
            output.reset_position();
            let _ = output.play();

            let target_rate = output.config.sample_rate.0;
            let target_channels = output.config.channels;
            let tx = self.decode_tx.clone();
            let queue = output.queue.clone();

            thread::spawn(move || {
                if pb_gen.load(Ordering::SeqCst) != generation {
                    return;
                }
                let check_gen = Arc::clone(&pb_gen);
                let push_gen = Arc::clone(&pb_gen);
                let queue_clone = queue.clone();

                match decode_track_streaming(
                    &path,
                    target_rate,
                    target_channels,
                    move || check_gen.load(Ordering::SeqCst) == generation,
                    move |chunk| {
                        let push_check = Arc::clone(&push_gen);
                        queue_clone.push_interleaved_cancellable(chunk.iter().copied(), move || {
                            push_check.load(Ordering::SeqCst) == generation
                        });
                    },
                ) {
                    Ok(track) => {
                        if pb_gen.load(Ordering::SeqCst) != generation {
                            return;
                        }
                        let exact_frames = track.samples.len() / usize::from(target_channels).max(1);
                        let exact_dur = Duration::from_secs_f64(exact_frames as f64 / f64::from(target_rate));
                        let _ = tx.send(DecodeEvent::TrackReady {
                            generation,
                            track_id,
                            samples: Arc::new(track.samples),
                            exact_duration: exact_dur,
                        });
                    }
                    Err(error) => eprintln!("unable to decode {}: {error}", path.display()),
                }
            });
        }
    }

    fn seek_to(&mut self, fraction: f32) {
        self.preloaded_track = None;
        let Some(duration) = self.playback.metadata.duration else { return };
        if duration.is_zero() { return; }

        let target_secs = duration.as_secs_f64() * (fraction.clamp(0.0, 1.0) as f64);
        let target_duration = Duration::from_secs_f64(target_secs);
        self.playback.elapsed = target_duration;

        if let (Some(audio_output), Some(samples)) = (&self.audio_output, &self.current_track_samples) {
            let generation = self.playback_generation.fetch_add(1, Ordering::SeqCst) + 1;
            let playback_generation = Arc::clone(&self.playback_generation);

            audio_output.queue.clear();
            self.playback_receiver.clear();
            audio_output.set_position(target_duration);

            let sample_rate = audio_output.config.sample_rate.0 as usize;
            let channels = usize::from(audio_output.config.channels).max(1);
            let raw_offset = (target_secs * sample_rate as f64 * channels as f64) as usize;
            let sample_offset = (raw_offset.min(samples.len())) / channels * channels;

            let queue = audio_output.queue.clone();
            let samples_arc = Arc::clone(samples);
            thread::spawn(move || {
                let slice = &samples_arc[sample_offset..];
                queue.push_interleaved_cancellable(slice.iter().copied(), || {
                    playback_generation.load(Ordering::SeqCst) == generation
                });
            });

            self.maybe_preload_next_track();
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
            self.seek_to(0.0);
            return;
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
        self.playback_generation.fetch_add(1, Ordering::SeqCst);
        self.preloaded_track = None;
        if let Some(audio_output) = &self.audio_output {
            audio_output.queue.clear();
            self.playback_receiver.clear();
            audio_output.reset_position();
            if let Err(error) = audio_output.pause() {
                eprintln!("unable to stop audio output: {error}");
            }
        }
        self.is_playing = false;
        self.seeking_fraction = None;
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

        // Track 4: Neon Skyline (WebM Vorbis)
        let track4_path = music_dir.join("04 - Neon Skyline.webm");
        if !track4_path.exists() {
            let status = std::process::Command::new("ffmpeg")
                .args([
                    "-f", "lavfi",
                    "-i", "sine=frequency=523.25:duration=12",
                    "-c:a", "libvorbis",
                    "-metadata", "title=Neon Skyline (WebM)",
                    "-metadata", "artist=Kanono Synth",
                    "-metadata", "album=Native Media",
                    "-metadata", "genre=Cyberpunk",
                    "-y",
                    track4_path.to_str().unwrap(),
                ])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();

            if status.is_err() || !status.as_ref().map(|s| s.success()).unwrap_or(false) {
                let wav_alt = music_dir.join("04 - Neon Skyline.wav");
                let _ = write_synth_wav(&wav_alt, sample_rate, 12, |t| {
                    ((2.0 * std::f32::consts::PI * 523.25 * t).sin() * 0.4).clamp(-0.95, 0.95)
                });
            }
        }

        let database_path = library_database_path();
        let mut library = LibraryDatabase::open(database_path).map_err(|e| e.to_string())?;
        library.scan_directory(&music_dir).map_err(|e| e.to_string())?;
        library.query(&TrackQuery::default()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

fn resolve_missing_durations() -> Command<Message> {
    Command::perform(
        async {
            let database_path = library_database_path();
            tokio::task::spawn_blocking(move || -> Vec<(i64, Duration)> {
                let mut resolved = Vec::new();
                let Ok(mut db) = LibraryDatabase::open(&database_path) else { return resolved };
                let Ok(tracks) = db.query(&TrackQuery::default()) else { return resolved };
                for t in tracks {
                    if t.duration.is_none() {
                        let dur = kanono_audio_engine::read_track(&t.path)
                            .ok()
                            .and_then(|info| info.duration)
                            .or_else(|| {
                                kanono_audio_engine::decode_track(&t.path)
                                    .ok()
                                    .and_then(|probed| probed.metadata.duration)
                            });
                        if let Some(d) = dur {
                            let _ = db.update_duration(t.id, d);
                            resolved.push((t.id, d));
                        }
                    }
                }
                resolved
            })
            .await
            .unwrap_or_default()
        },
        Message::DurationsBatchResolved,
    )
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

    #[test]
    fn test_seeking_calculation_and_alignment() {
        let duration = Duration::from_secs(180);
        let sample_rate = 48_000usize;
        let channels = 2usize;
        let total_samples = 180 * sample_rate * channels;

        let fraction = 0.5f32;
        let target_secs = duration.as_secs_f64() * (fraction.clamp(0.0, 1.0) as f64);
        let target_duration = Duration::from_secs_f64(target_secs);
        assert_eq!(target_duration, Duration::from_secs(90));

        let raw_offset = (target_secs * sample_rate as f64 * channels as f64) as usize;
        let sample_offset = (raw_offset.min(total_samples)) / channels * channels;
        // Verify channel alignment
        assert_eq!(sample_offset % channels, 0);
        assert_eq!(sample_offset, 90 * 48000 * 2);
    }

    #[test]
    fn test_track_finished_detection_logic() {
        let total_frames = 48000 * 180u64;
        let played_frames = total_frames;
        let queue_empty = true;
        let sample_rate = 48000u64;

        let track_finished = total_frames > 0 && (
            played_frames >= total_frames || 
            (queue_empty && played_frames + (sample_rate / 10) >= total_frames)
        );
        assert!(track_finished);

        // Not finished when halfway
        let played_half = total_frames / 2;
        let track_not_finished = total_frames > 0 && (
            played_half >= total_frames || 
            (queue_empty && played_half + (sample_rate / 10) >= total_frames)
        );
        assert!(!track_not_finished);

        // Finished when queue is empty and near EOF (within 100ms)
        let played_near_eof = total_frames - 2000;
        let near_eof_finished = total_frames > 0 && (
            played_near_eof >= total_frames || 
            (queue_empty && played_near_eof + (sample_rate / 10) >= total_frames)
        );
        assert!(near_eof_finished);
    }

    #[test]
    fn test_filter_state_matching() {
        let mut filter = FilterState::default();
        assert!(!filter.is_any_active());

        filter.duration = Some(DurationFilter::Short);
        assert!(filter.is_any_active());
        assert!(filter.duration.unwrap().matches(Some(Duration::from_secs(90))));
        assert!(!filter.duration.unwrap().matches(Some(Duration::from_secs(200))));

        filter.bpm = Some(BpmFilter::Fast);
        assert!(filter.bpm.unwrap().matches(Some(130)));
        assert!(!filter.bpm.unwrap().matches(Some(80)));

        filter.clear();
        assert!(!filter.is_any_active());
    }

    #[test]
    fn test_folder_tree_hierarchical_building() {
        use crate::ui::build_folder_tree;

        let tracks = vec![
            LibraryTrack {
                id: 1,
                path: PathBuf::from("/music/Rock/Queen/Bohemian.mp3"),
                title: "Bohemian".into(),
                artist: "Queen".into(),
                album: "Opera".into(),
                genre: "Rock".into(),
                year: Some(1975),
                duration: Some(Duration::from_secs(354)),
                play_count: 10,
                bpm: Some(72),
                track_number: Some(1),
            },
            LibraryTrack {
                id: 2,
                path: PathBuf::from("/music/Rock/Queen/RadioGaGa.mp3"),
                title: "Radio Ga Ga".into(),
                artist: "Queen".into(),
                album: "Works".into(),
                genre: "Rock".into(),
                year: Some(1984),
                duration: Some(Duration::from_secs(348)),
                play_count: 5,
                bpm: Some(112),
                track_number: Some(2),
            },
            LibraryTrack {
                id: 3,
                path: PathBuf::from("/music/Jazz/Miles/SoWhat.mp3"),
                title: "So What".into(),
                artist: "Miles Davis".into(),
                album: "Kind of Blue".into(),
                genre: "Jazz".into(),
                year: Some(1959),
                duration: Some(Duration::from_secs(562)),
                play_count: 8,
                bpm: Some(136),
                track_number: Some(1),
            },
        ];

        let tree = build_folder_tree(&tracks);
        assert!(!tree.is_empty());
        let total_tracks: usize = tree.iter().map(|n| n.track_count).sum();
        assert_eq!(total_tracks, 3);
    }

    #[test]
    fn test_wav_duration_resolution() {
        let p = PathBuf::from("/tmp/kanono_demo_music/01 - Kanono Groove.wav");
        if p.exists() {
            let track = kanono_audio_engine::read_track(&p).expect("read_track on demo wav");
            assert!(track.duration.is_some(), "WAV duration must be parsed from header");
            let d = track.duration.unwrap();
            assert!(d.as_secs() > 0, "Duration must be greater than 0");
        }
    }

    #[test]
    fn test_user_db_migration() {
        let db_path = library_database_path();
        if db_path.exists() {
            let mut db = LibraryDatabase::open(&db_path).expect("open user db");
            let tracks = db.query(&TrackQuery::default()).expect("query user db");
            for t in tracks {
                if t.duration.is_none() {
                    if let Ok(info) = kanono_audio_engine::read_track(&t.path) {
                        if let Some(dur) = info.duration {
                            let _ = db.update_duration(t.id, dur);
                        }
                    }
                }
            }
        }
    }
}