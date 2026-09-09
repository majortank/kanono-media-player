use std::{collections::BTreeSet, path::PathBuf, time::Duration};

use iced::{widget::{button, column, container, row, scrollable, text, text_input}, Alignment, Element, Length};
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
        text("LIBRARY").size(14),
        button("All Tracks").on_press(Message::SelectFolder(None)).width(Length::Fill),
    ].spacing(8);
    for folder in folders {
        let label = folder.file_name().and_then(|name| name.to_str()).unwrap_or("Music").to_owned();
        sidebar = sidebar.push(button(text(label)).on_press(Message::SelectFolder(Some(folder))).width(Length::Fill));
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
    let library_title = selected_folder.and_then(|folder| folder.file_name()).and_then(|name| name.to_str()).unwrap_or("All Tracks").to_owned();
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
    ].spacing(10).align_items(Alignment::Center).padding(12)).width(Length::Fill);
    container(column![row![sidebar, container(column![selected_actions, content].spacing(12).padding(16)).width(Length::Fill).height(Length::Fill)], transport].height(Length::Fill)).into()
}

fn folder_roots(tracks: &[LibraryTrack]) -> Vec<PathBuf> {
    tracks.iter().filter_map(|track| track.path.parent().map(PathBuf::from)).collect::<BTreeSet<_>>().into_iter().collect()
}

fn format_duration(duration: Duration) -> String {
    format!("{:02}:{:02}", duration.as_secs() / 60, duration.as_secs() % 60)
}
