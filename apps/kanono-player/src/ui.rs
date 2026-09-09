use std::{collections::BTreeSet, path::PathBuf, time::Duration};

use iced::{widget::{button, column, container, mouse_area, row, scrollable, text, text_input, Column, Row}, Alignment, Element, Length};
use kanono_audio_engine::{LibraryTrack, TrackMetadata};

use crate::Message;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel { Playlist, Visualizer, TrackInfo }

impl Panel {
    fn label(self) -> &'static str {
        match self {
            Self::Playlist => "Playlist",
            Self::Visualizer => "Visualizer",
            Self::TrackInfo => "Track Info",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitAxis { Horizontal, Vertical }

pub struct LayoutGrid {
    playlist: bool,
    visualizer: bool,
    track_info: bool,
    slots: [Panel; 3],
    axis: SplitAxis,
    design_mode: bool,
    dragging: Option<Panel>,
}

impl Default for LayoutGrid {
    fn default() -> Self {
        Self {
            playlist: true,
            visualizer: true,
            track_info: true,
            slots: [Panel::Playlist, Panel::Visualizer, Panel::TrackInfo],
            axis: SplitAxis::Horizontal,
            design_mode: false,
            dragging: None,
        }
    }
}

impl LayoutGrid {
    pub fn toggle(&mut self, panel: Panel) {
        match panel {
            Panel::Playlist => self.playlist = !self.playlist,
            Panel::Visualizer => self.visualizer = !self.visualizer,
            Panel::TrackInfo => self.track_info = !self.track_info,
        }
    }

    pub fn toggle_design_mode(&mut self) {
        self.design_mode = !self.design_mode;
        self.dragging = None;
    }

    pub fn set_axis(&mut self, axis: SplitAxis) {
        self.axis = axis;
    }

    pub fn begin_drag(&mut self, panel: Panel) {
        if self.design_mode {
            self.dragging = Some(panel);
        }
    }

    pub fn drop_on(&mut self, target: Panel) {
        let Some(source) = self.dragging.take() else { return };
        if source == target { return; }
        let source_slot = self.slots.iter().position(|panel| *panel == source).expect("all panels occupy one slot");
        let target_slot = self.slots.iter().position(|panel| *panel == target).expect("all panels occupy one slot");
        self.slots.swap(source_slot, target_slot);
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
            button(if self.design_mode { "Exit Design" } else { "Design" }).on_press(Message::ToggleDesignMode),
        ].spacing(8);
        let editor = if self.design_mode {
            row![
                text("Design Mode: drag a panel header onto another header to swap"),
                button("Horizontal Split").on_press(Message::SetSplitAxis(SplitAxis::Horizontal)),
                button("Vertical Split").on_press(Message::SetSplitAxis(SplitAxis::Vertical)),
            ].spacing(8).align_items(Alignment::Center)
        } else {
            row![].spacing(0)
        };
        let mut panel_elements = Vec::new();
        for panel in self.slots {
            let content = match panel {
                Panel::Playlist if self.playlist => Some(playlist()),
                Panel::Visualizer if self.visualizer => Some(visualizer()),
                Panel::TrackInfo if self.track_info => Some(track_info()),
                _ => None,
            };
            if let Some(content) = content {
                panel_elements.push(self.panel_slot(panel, content));
            }
        }
        let panels: Element<'a, Message> = match self.axis {
            SplitAxis::Horizontal => Row::with_children(panel_elements).spacing(12).into(),
            SplitAxis::Vertical => Column::with_children(panel_elements).spacing(12).into(),
        };
        container(column![controls, editor, panels].spacing(12).padding(16).align_items(Alignment::Start))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn panel_slot<'a>(&self, panel: Panel, content: Element<'a, Message>) -> Element<'a, Message> {
        if !self.design_mode {
            return container(content).width(Length::FillPortion(1)).into();
        }
        let header = container(text(panel.label())).padding(8).width(Length::Fill);
        mouse_area(column![header, content].spacing(6))
            .on_press(Message::BeginPanelDrag(panel))
            .on_release(Message::DropPanelOn(panel))
            .into()
    }
}

pub fn player_view<'a>(
    tracks: &'a [LibraryTrack],
    selected_folder: Option<&'a PathBuf>,
    search: &'a str,
    selected_track: Option<i64>,
    queued_tracks: usize,
    is_playing: bool,
    metadata: &'a TrackMetadata,
    elapsed: Duration,
) -> Element<'a, Message> {
    let folders = folder_roots(tracks);
    let mut sidebar = column![
        text("LIBRARY").size(14),
        button("All Tracks").on_press(Message::SelectFolder(None)).width(Length::Fill),
    ].spacing(8);
    for folder in folders {
        let label = folder.file_name().and_then(|name| name.to_str()).unwrap_or("Music");
        sidebar = sidebar.push(button(label).on_press(Message::SelectFolder(Some(folder))).width(Length::Fill));
    }
    let sidebar = container(scrollable(sidebar.padding(12))).width(Length::Fixed(220.0)).height(Length::Fill);

    let filter = text_input("Search title, artist, album...", search)
        .on_input(Message::SearchChanged)
        .padding(10)
        .width(Length::Fill);
    let mut rows = column![row![text("TITLE").width(Length::FillPortion(4)), text("ARTIST").width(Length::FillPortion(3)), text("ALBUM").width(Length::FillPortion(3)), text("TIME").width(Length::Fixed(56.0))].spacing(10)].spacing(6);
    for track in tracks {
        let active = selected_track == Some(track.id);
        let title = if active { format!("> {}", track.title) } else { track.title.clone() };
        let duration = track.duration.map(format_duration).unwrap_or_else(|| "--:--".into());
        let track_id = track.id;
        rows = rows.push(button(row![
            text(title).width(Length::FillPortion(4)),
            text(&track.artist).width(Length::FillPortion(3)),
            text(&track.album).width(Length::FillPortion(3)),
            text(duration).width(Length::Fixed(56.0)),
        ].spacing(10)).on_press(Message::SelectTrack(track_id)).width(Length::Fill));
    }
    let library_title = selected_folder.and_then(|folder| folder.file_name()).and_then(|name| name.to_str()).unwrap_or("All Tracks");
    let content = if tracks.is_empty() {
        column![
            text("Your library is empty").size(28),
            text("Add a music folder to index MP3, FLAC, and WAV files."),
            button("Add Music Folder").on_press(Message::ImportFolder),
        ].spacing(14).align_items(Alignment::Center).width(Length::Fill).height(Length::Fill)
    } else {
        column![text(library_title).size(26), filter, scrollable(rows).height(Length::Fill)].spacing(12).width(Length::Fill).height(Length::Fill)
    };
    let selected_actions = match selected_track {
        Some(track_id) => row![button("Play Selected").on_press(Message::PlayTrack(track_id)), button("Add to Queue").on_press(Message::QueueTrack(track_id))].spacing(8),
        None => row![].spacing(0),
    };
    let now_playing = if metadata.title.is_empty() { "Nothing playing" } else { &metadata.title };
    let transport = container(row![
        button("Previous").on_press(Message::Previous),
        button(if is_playing { "Pause" } else { "Play" }).on_press(Message::TogglePlayback),
        button("Next").on_press(Message::Next),
        column![text(now_playing), text(format!("{}  |  {} queued", format_duration(elapsed), queued_tracks))].spacing(2).width(Length::Fill),
        button("Add Folder").on_press(Message::ImportFolder),
        button("Layout").on_press(Message::ToggleDesignMode),
    ].spacing(10).align_items(Alignment::Center).padding(12)).width(Length::Fill);
    container(column![row![sidebar, container(column![selected_actions, content].spacing(12).padding(16)).width(Length::Fill).height(Length::Fill)], transport].height(Length::Fill)).into()
}

fn folder_roots(tracks: &[LibraryTrack]) -> Vec<PathBuf> {
    tracks.iter().filter_map(|track| track.path.parent().map(PathBuf::from)).collect::<BTreeSet<_>>().into_iter().collect()
}

fn format_duration(duration: Duration) -> String {
    format!("{:02}:{:02}", duration.as_secs() / 60, duration.as_secs() % 60)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn design_mode_swaps_panel_slots() {
        let mut layout = LayoutGrid::default();
        layout.toggle_design_mode();
        layout.begin_drag(Panel::Playlist);
        layout.drop_on(Panel::TrackInfo);

        assert_eq!(layout.slots, [Panel::TrackInfo, Panel::Visualizer, Panel::Playlist]);
    }
}