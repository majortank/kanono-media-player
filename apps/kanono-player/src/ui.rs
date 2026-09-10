use std::{collections::BTreeSet, path::PathBuf, time::Duration};

use iced::{widget::{button, column, container, row, scrollable, text, text_input}, Alignment, Color, Element, Length};
use kanono_audio_engine::{LibraryTrack, TrackMetadata};

use crate::Message;

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
        text("LIBRARY").size(12).letter_spacing(1.6),
        button("All Tracks").on_press(Message::SelectFolder(None)).width(Length::Fill),
    ].spacing(8);
    for folder in folders {
        let label = folder.file_name().and_then(|name| name.to_str()).unwrap_or("Music").to_owned();
        sidebar = sidebar.push(button(text(label)).on_press(Message::SelectFolder(Some(folder))).width(Length::Fill));
    }
    let sidebar = container(scrollable(sidebar.padding(12))).width(Length::Fixed(236.0)).height(Length::Fill).style(surface_style());

    let filter = text_input("Search title, artist, album...", search)
        .on_input(Message::SearchChanged)
        .padding(10)
        .width(Length::Fill);
    let mut rows = column![row![
        text("TITLE").width(Length::FillPortion(4)).color(Color::from_rgb8(173, 181, 197)),
        text("ARTIST").width(Length::FillPortion(3)).color(Color::from_rgb8(173, 181, 197)),
        text("ALBUM").width(Length::FillPortion(3)).color(Color::from_rgb8(173, 181, 197)),
        text("TIME").width(Length::Fixed(56.0)).color(Color::from_rgb8(173, 181, 197)),
    ].spacing(10)].spacing(6);
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
        ].spacing(10).align_items(Alignment::Center)).on_press(Message::SelectTrack(track_id)).width(Length::Fill).style(if active { accent_button_style() } else { list_button_style() }));
    }
    let library_title = selected_folder.and_then(|folder| folder.file_name()).and_then(|name| name.to_str()).unwrap_or("All Tracks").to_owned();
    let content = if tracks.is_empty() {
        container(
            column![
                text("Your library is empty").size(28).color(Color::WHITE),
                text("Add a music folder to index MP3, FLAC, and WAV files.").color(Color::from_rgb8(177, 186, 201)),
                button("Add Music Folder").on_press(Message::ImportFolder).style(primary_button_style()),
            ].spacing(14).align_items(Alignment::Center).width(Length::Fill).height(Length::Fill)
        )
        .padding(30)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(surface_style())
    } else {
        container(
            column![
                row![
                    text(&library_title).size(26).width(Length::Fill),
                    container(text(format!("{} queued", queued_tracks)).size(12).color(Color::from_rgb8(184, 196, 214))).padding([6, 10]).style(tag_style())
                ].align_items(Alignment::Center),
                filter,
                scrollable(rows).height(Length::Fill),
            ].spacing(12).width(Length::Fill).height(Length::Fill),
        )
        .padding(16)
        .style(surface_style())
    };
    let selected_actions = match selected_track {
        Some(track_id) => row![
            button("Play Selected").on_press(Message::PlayTrack(track_id)).style(primary_button_style()),
            button("Add to Queue").on_press(Message::QueueTrack(track_id)).style(default_button_style()),
        ].spacing(8),
        None => row![].spacing(0),
    };
    let now_playing = if metadata.title.is_empty() { "Nothing playing" } else { &metadata.title };
    let transport = container(
        row![
            button("⏮").on_press(Message::Previous).style(default_button_style()),
            button(if is_playing { "⏸" } else { "▶" }).on_press(Message::TogglePlayback).style(primary_button_style()),
            button("⏭").on_press(Message::Next).style(default_button_style()),
            column![
                text(now_playing).size(18).color(Color::WHITE),
                text(format!("{}  |  {} queued", format_duration(elapsed), queued_tracks)).size(12).color(Color::from_rgb8(181, 190, 204)),
            ].spacing(2).width(Length::Fill),
            button("Add Folder").on_press(Message::ImportFolder).style(default_button_style()),
        ].spacing(10).align_items(Alignment::Center).padding(12),
    )
    .width(Length::Fill)
    .style(surface_style());
    container(column![row![sidebar, container(column![selected_actions, content].spacing(12).padding(16)).width(Length::Fill).height(Length::Fill)], transport].height(Length::Fill)).into()
}

fn folder_roots(tracks: &[LibraryTrack]) -> Vec<PathBuf> {
    tracks.iter().filter_map(|track| track.path.parent().map(PathBuf::from)).collect::<BTreeSet<_>>().into_iter().collect()
}

fn format_duration(duration: Duration) -> String {
    format!("{:02}:{:02}", duration.as_secs() / 60, duration.as_secs() % 60)
}

fn surface_style() -> iced::theme::Container {
    iced::theme::Container::Custom(Box::new(PanelStyle))
}

fn primary_button_style() -> iced::theme::Button {
    iced::theme::Button::Custom(Box::new(PrimaryButtonStyle))
}

fn default_button_style() -> iced::theme::Button {
    iced::theme::Button::Custom(Box::new(DefaultButtonStyle))
}

fn accent_button_style() -> iced::theme::Button {
    iced::theme::Button::Custom(Box::new(AccentButtonStyle))
}

fn list_button_style() -> iced::theme::Button {
    iced::theme::Button::Custom(Box::new(ListButtonStyle))
}

fn tag_style() -> iced::theme::Container {
    iced::theme::Container::Custom(Box::new(TagStyle))
}

struct PanelStyle;

impl iced::widget::container::StyleSheet for PanelStyle {
    type Style = iced::Theme;

    fn appearance(&self, _style: &Self::Style) -> iced::widget::container::Appearance {
        iced::widget::container::Appearance {
            background: Some(Color::from_rgb8(18, 22, 33).into()),
            text_color: Some(Color::WHITE),
            border_radius: 14.0,
            border_width: 1.0,
            border_color: Color::from_rgb8(40, 48, 63),
        }
    }
}

struct PrimaryButtonStyle;

impl iced::widget::button::StyleSheet for PrimaryButtonStyle {
    type Style = iced::Theme;

    fn appearance(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(Color::from_rgb8(76, 166, 122).into()),
            text_color: Color::WHITE,
            border_radius: 10.0,
            border_width: 1.0,
            border_color: Color::from_rgb8(104, 196, 151),
            shadow_offset: iced::Vector::new(0.0, 2.0),
            shadow_blur: 8.0,
            shadow_color: Color::from_rgba8(76, 166, 122, 0.3),
        }
    }
}

struct DefaultButtonStyle;

impl iced::widget::button::StyleSheet for DefaultButtonStyle {
    type Style = iced::Theme;

    fn appearance(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(Color::from_rgb8(30, 35, 47).into()),
            text_color: Color::WHITE,
            border_radius: 10.0,
            border_width: 1.0,
            border_color: Color::from_rgb8(54, 63, 79),
            shadow_offset: iced::Vector::new(0.0, 0.0),
            shadow_blur: 0.0,
            shadow_color: Color::TRANSPARENT,
        }
    }
}

struct AccentButtonStyle;

impl iced::widget::button::StyleSheet for AccentButtonStyle {
    type Style = iced::Theme;

    fn appearance(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(Color::from_rgb8(46, 104, 160).into()),
            text_color: Color::WHITE,
            border_radius: 10.0,
            border_width: 1.0,
            border_color: Color::from_rgb8(90, 147, 211),
            shadow_offset: iced::Vector::new(0.0, 0.0),
            shadow_blur: 0.0,
            shadow_color: Color::TRANSPARENT,
        }
    }
}

struct ListButtonStyle;

impl iced::widget::button::StyleSheet for ListButtonStyle {
    type Style = iced::Theme;

    fn appearance(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(Color::from_rgb8(19, 24, 36).into()),
            text_color: Color::WHITE,
            border_radius: 8.0,
            border_width: 1.0,
            border_color: Color::from_rgb8(38, 45, 59),
            shadow_offset: iced::Vector::new(0.0, 0.0),
            shadow_blur: 0.0,
            shadow_color: Color::TRANSPARENT,
        }
    }
}

struct TagStyle;

impl iced::widget::container::StyleSheet for TagStyle {
    type Style = iced::Theme;

    fn appearance(&self, _style: &Self::Style) -> iced::widget::container::Appearance {
        iced::widget::container::Appearance {
            background: Some(Color::from_rgb8(33, 41, 55).into()),
            text_color: Some(Color::from_rgb8(184, 196, 214)),
            border_radius: 999.0,
            border_width: 1.0,
            border_color: Color::from_rgb8(53, 63, 79),
        }
    }
}
