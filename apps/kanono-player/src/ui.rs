use std::time::Duration;

use iced::{widget::{button, column, container, row, text}, Alignment, Element, Length};
use kanono_audio_engine::TrackMetadata;

use crate::Message;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel { Playlist, Visualizer, TrackInfo }

#[derive(Default)]
pub struct LayoutGrid {
    playlist: bool,
    visualizer: bool,
    track_info: bool,
}

impl LayoutGrid {
    pub fn toggle(&mut self, panel: Panel) {
        match panel {
            Panel::Playlist => self.playlist = !self.playlist,
            Panel::Visualizer => self.visualizer = !self.visualizer,
            Panel::TrackInfo => self.track_info = !self.track_info,
        }
    }

    pub fn view<'a>(
        &'a self,
        playlist: impl Fn() -> Element<'static, Message>,
        visualizer: impl Fn() -> Element<'a, Message>,
        track_info: impl Fn() -> Element<'a, Message>,
    ) -> Element<'a, Message> {
        let controls = row![
            button("Playlist").on_press(Message::Toggle(Panel::Playlist)),
            button("Visualizer").on_press(Message::Toggle(Panel::Visualizer)),
            button("Track Info").on_press(Message::Toggle(Panel::TrackInfo)),
        ].spacing(8);
        let mut panels = column![controls].spacing(12).padding(16).align_items(Alignment::Start);
        if self.playlist { panels = panels.push(playlist()); }
        if self.visualizer { panels = panels.push(visualizer()); }
        if self.track_info { panels = panels.push(track_info()); }
        container(panels).width(Length::Fill).height(Length::Fill).into()
    }
}

pub struct Playlist;
impl Playlist {
    pub fn view() -> Element<'static, Message> {
        container(column![text("Playlist"), text("No tracks queued")].spacing(4)).into()
    }
}

pub struct Visualizer;
impl Visualizer {
    pub fn view(samples: &[f32]) -> Element<'_, Message> {
        let peak = samples.iter().fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
        container(column![text("Visualizer"), text(format!("{} PCM samples | peak {:.3}", samples.len(), peak))].spacing(4)).into()
    }
}

pub struct TrackInfo;
impl TrackInfo {
    pub fn view(metadata: &TrackMetadata, elapsed: Duration) -> Element<'_, Message> {
        let title = if metadata.title.is_empty() { "Nothing playing" } else { &metadata.title };
        container(column![text("Track Info"), text(title), text(format!("{} - {} | {:02}:{:02}", metadata.artist, metadata.album, elapsed.as_secs() / 60, elapsed.as_secs() % 60))].spacing(4)).into()
    }
}