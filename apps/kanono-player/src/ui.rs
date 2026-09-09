use std::time::Duration;

use iced::{widget::{button, column, container, mouse_area, row, text, Column, Row}, Alignment, Element, Length};
use kanono_audio_engine::TrackMetadata;

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