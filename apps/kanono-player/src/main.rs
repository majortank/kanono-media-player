mod plugins;
mod ui;

use iced::{executor, Application, Command, Element, Theme};
use ui::{LayoutGrid, Panel, Playlist, TrackInfo, Visualizer};

fn main() -> iced::Result {
    KanonoApp::run(iced::Settings::default())
}

struct KanonoApp {
    layout: LayoutGrid,
}

#[derive(Debug, Clone)]
enum Message {
    Toggle(Panel),
}

impl Application for KanonoApp {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        (Self { layout: LayoutGrid::default() }, Command::none())
    }

    fn title(&self) -> String {
        "Kanono Media Player".into()
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::Toggle(panel) => self.layout.toggle(panel),
        }
        Command::none()
    }

    fn view(&self) -> Element<'_, Message> {
        self.layout.view(Playlist::view, Visualizer::view, TrackInfo::view)
    }
}