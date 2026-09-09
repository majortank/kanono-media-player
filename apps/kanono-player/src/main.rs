mod plugins;
mod ui;

use std::{path::PathBuf, time::Duration};

use iced::{executor, time, Application, Command, Element, Subscription, Theme};
use kanono_audio_engine::{playback_state_channel, PlaybackStateReceiver, PlaybackUpdate, TrackMetadata};
use plugins::PluginRegistry;
use ui::{LayoutGrid, Panel, Playlist, SplitAxis, TrackInfo, Visualizer};

fn main() -> iced::Result {
    KanonoApp::run(iced::Settings::default())
}

struct KanonoApp {
    layout: LayoutGrid,
    playback: PlaybackViewState,
    playback_receiver: PlaybackStateReceiver,
    components: PluginRegistry,
}

#[derive(Default)]
struct PlaybackViewState {
    metadata: TrackMetadata,
    elapsed: Duration,
    visualizer_pcm: Vec<f32>,
}

#[derive(Debug, Clone)]
enum Message {
    Toggle(Panel),
    ToggleDesignMode,
    BeginPanelDrag(Panel),
    DropPanelOn(Panel),
    SetSplitAxis(SplitAxis),
    PollPlayback,
}

impl Application for KanonoApp {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let (_playback_sender, playback_receiver) = playback_state_channel();
        let components = PluginRegistry::load_components(component_directory());
        (
            Self {
                layout: LayoutGrid::default(),
                playback: PlaybackViewState::default(),
                playback_receiver,
                components,
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
            Message::Toggle(panel) => self.layout.toggle(panel),
            Message::ToggleDesignMode => self.layout.toggle_design_mode(),
            Message::BeginPanelDrag(panel) => self.layout.begin_drag(panel),
            Message::DropPanelOn(panel) => self.layout.drop_on(panel),
            Message::SetSplitAxis(axis) => self.layout.set_axis(axis),
            Message::PollPlayback => self.apply_playback_updates(),
        }
        Command::none()
    }

    fn view(&self) -> Element<'_, Message> {
        self.layout.view(
            Playlist::view,
            || Visualizer::view(&self.playback.visualizer_pcm),
            || TrackInfo::view(&self.playback.metadata, self.playback.elapsed),
        )
    }

    fn subscription(&self) -> Subscription<Message> {
        time::every(Duration::from_millis(33)).map(|_| Message::PollPlayback)
    }
}

fn component_directory() -> PathBuf {
    std::env::var_os("KANONO_COMPONENTS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("components"))
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
    }
}