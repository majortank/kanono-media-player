use std::{collections::BTreeSet, path::PathBuf, time::Duration};

use iced::{
    widget::{button, column, container, horizontal_space, row, scrollable, slider, svg, text, text_input},
    Alignment, Color, Element, Length,
};
use kanono_audio_engine::{LibraryTrack, ReplayGainResult, TrackMetadata};

use crate::{plugins::RegisteredPlugin, EditField, EditingTrackState, Message, NavTab};

const LOGO_SVG: &[u8] = include_bytes!("../../../assets/icons/hicolor/scalable/apps/kanono-media-player.svg");
const ICON_PLAY_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#ffffff"><polygon points="6,4 20,12 6,20"/></svg>"##;
const ICON_PAUSE_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#ffffff"><rect x="5" y="4" width="4" height="16"/><rect x="15" y="4" width="4" height="16"/></svg>"##;
const ICON_PREV_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#ffffff"><rect x="4" y="5" width="2.5" height="14"/><polygon points="20,5 8.5,12 20,19"/></svg>"##;
const ICON_NEXT_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#ffffff"><polygon points="4,5 15.5,12 4,19"/><rect x="17.5" y="5" width="2.5" height="14"/></svg>"##;
const ICON_SHUFFLE_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#cbd5e1"><path d="M10.59 9.17L5.41 4 4 5.41l5.17 5.17 1.42-1.41zM14.5 4l2.04 2.04L4 18.59 5.41 20 17.96 7.46 20 9.5V4h-5.5zm.33 9.41l-1.41 1.41 3.13 3.13L14.5 20H20v-5.5l-2.04 2.04-3.13-3.13z"/></svg>"##;
const ICON_REPEAT_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#cbd5e1"><path d="M7 7h10v3l4-4-4-4v3H5v6h2V7zm10 10H7v-3l-4 4 4 4v-3h12v-6h-2v4z"/></svg>"##;
const ICON_VOLUME_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#cbd5e1"><path d="M3 9v6h4l5 5V4L7 9H3zm13.5 3c0-1.77-1.02-3.29-2.5-4.03v8.05c1.48-.73 2.5-2.25 2.5-4.02zM14 3.23v2.06c2.89.86 5 3.54 5 6.71s-2.11 5.85-5 6.71v2.06c4.01-.91 7-4.49 7-8.77s-2.99-7.86-7-8.77z"/></svg>"##;
const ICON_MUTE_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#f87171"><path d="M16.5 12c0-1.77-1.02-3.29-2.5-4.03v2.21l2.45 2.45c.03-.2.05-.41.05-.63zm2.5 0c0 .94-.2 1.82-.54 2.64l1.51 1.51C20.63 14.91 21 13.5 21 12c0-4.28-2.99-7.86-7-8.77v2.06c2.89.86 5 3.54 5 6.71zM4.27 3L3 4.27 7.73 9H3v6h4l5 5v-6.73l4.25 4.25c-.67.52-1.42.93-2.25 1.18v2.06c1.38-.31 2.63-.95 3.69-1.81L19.73 21 21 19.73l-9-9L4.27 3zM12 4L9.91 6.09 12 8.18V4z"/></svg>"##;
const ICON_DISC_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><circle cx="12" cy="12" r="10" fill="#10b981"/><circle cx="12" cy="12" r="3.5" fill="#0f172a"/><circle cx="12" cy="12" r="1.5" fill="#6ee7b7"/></svg>"##;
const ICON_FOLDER_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#94a3b8"><path d="M10 4H4c-1.1 0-1.99.9-1.99 2L2 18c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2h-8l-2-2z"/></svg>"##;
const ICON_LIBRARY_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#94a3b8"><path d="M4 6H2v14c0 1.1.9 2 2 2h14v-2H4V6zm16-4H8c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V4c0-1.1-.9-2-2-2zm0 14H8V4h12v12z"/></svg>"##;
const ICON_QUEUE_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#94a3b8"><path d="M15 6H3v2h12V6zm0 4H3v2h12v-2zM3 16h8v-2H3v2zM17 6v8.18c-.31-.11-.65-.18-1-.18-1.66 0-3 1.34-3 3s1.34 3 3 3 3-1.34 3-3V8h3V6h-5z"/></svg>"##;
const ICON_INFO_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#94a3b8"><path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-6h2v6zm0-8h-2V7h2v2z"/></svg>"##;
const ICON_EDIT_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#94a3b8"><path d="M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25zM20.71 7.04c.39-.39.39-1.02 0-1.41l-2.34-2.34c-.39-.39-1.02-.39-1.41 0l-1.83 1.83 3.75 3.75 1.83-1.83z"/></svg>"##;

fn svg_icon<'a, M: 'a>(data: &'static [u8], size: f32) -> Element<'a, M> {
    svg(svg::Handle::from_memory(data))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .into()
}

pub struct ViewProps<'a> {
    pub tracks: &'a [LibraryTrack],
    pub all_tracks: &'a [LibraryTrack],
    pub selected_folder: Option<&'a PathBuf>,
    pub current_tab: NavTab,
    pub search: &'a str,
    pub selected_track: Option<i64>,
    pub current_playing_track_id: Option<i64>,
    pub queued_track_ids: &'a [i64],
    pub is_playing: bool,
    pub is_shuffled: bool,
    pub is_repeated: bool,
    pub replaygain_enabled: bool,
    pub gapless_enabled: bool,
    pub volume: f32,
    pub is_muted: bool,
    pub metadata: &'a TrackMetadata,
    pub elapsed: Duration,
    pub status_message: Option<&'a str>,
    pub visualizer_pcm: &'a [f32],
    pub current_track_gain: Option<ReplayGainResult>,
    pub editing_track: Option<&'a EditingTrackState>,
    pub components: &'a [RegisteredPlugin],
}

pub fn player_view<'a>(props: ViewProps<'a>) -> Element<'a, Message> {
    // --- TOP BAR ---
    let logo_icon = svg(svg::Handle::from_memory(LOGO_SVG))
        .width(Length::Fixed(28.0))
        .height(Length::Fixed(28.0));

    let logo = row![
        logo_icon,
        text("KANONO").size(18).style(Color::from_rgb8(16, 185, 129)),
        text("PLAYER").size(18).style(Color::WHITE),
    ]
    .spacing(8)
    .align_items(Alignment::Center);

    let search_bar = text_input("Search music library by title, artist, album, genre...", props.search)
        .on_input(Message::SearchChanged)
        .padding(9)
        .width(Length::Fill);

    let top_actions = row![
        button(
            row![
                svg_icon(ICON_FOLDER_SVG, 15.0),
                text("Add Folder").size(13),
            ]
            .spacing(6)
            .align_items(Alignment::Center),
        )
        .on_press(Message::ImportFolder)
        .padding([8, 14])
        .style(btn_default_style()),

        button(
            row![
                svg_icon(ICON_PLAY_SVG, 13.0),
                text("Sample Audio").size(13).style(Color::WHITE),
            ]
            .spacing(6)
            .align_items(Alignment::Center),
        )
        .on_press(Message::GenerateSampleAudio)
        .padding([8, 14])
        .style(btn_accent_style()),
    ]
    .spacing(8)
    .align_items(Alignment::Center);

    let mut top_row = row![logo, search_bar].spacing(16).align_items(Alignment::Center);
    if let Some(status) = props.status_message {
        top_row = top_row.push(
            container(text(status).size(12).style(Color::from_rgb8(52, 211, 153)))
                .padding([6, 10])
                .style(badge_container_style()),
        );
    }
    top_row = top_row.push(top_actions);

    let top_bar = container(top_row)
        .padding([10, 16])
        .width(Length::Fill)
        .style(panel_container_style());

    // --- LEFT SIDEBAR ---
    let sidebar = render_sidebar(&props);

    // --- MAIN CONTENT ---
    let main_content = match props.current_tab {
        NavTab::Library | NavTab::Folders => render_track_list(&props),
        NavTab::Queue => render_queue_list(&props),
        NavTab::Info => render_info_view(&props),
    };

    let middle = row![sidebar, main_content]
        .spacing(12)
        .width(Length::Fill)
        .height(Length::Fill);

    // --- BOTTOM TRANSPORT BAR ---
    let transport = render_transport_bar(&props);

    container(
        column![top_bar, middle, transport]
            .spacing(10)
            .padding(10)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .style(root_background_style())
    .into()
}

fn render_sidebar<'a>(props: &ViewProps<'a>) -> Element<'a, Message> {
    let folders = folder_roots(props.all_tracks);

    let is_lib_active = props.current_tab == NavTab::Library && props.selected_folder.is_none();
    let lib_btn = button(
        row![
            svg_icon(ICON_LIBRARY_SVG, 16.0),
            text("All Tracks").size(14).width(Length::Fill),
            text(format!("{}", props.all_tracks.len())).size(11).style(Color::from_rgb8(148, 163, 184)),
        ]
        .spacing(8)
        .align_items(Alignment::Center),
    )
    .on_press(Message::SelectTab(NavTab::Library))
    .width(Length::Fill)
    .padding([8, 12])
    .style(if is_lib_active { btn_selected_nav_style() } else { btn_nav_style() });

    let is_queue_active = props.current_tab == NavTab::Queue;
    let queue_btn = button(
        row![
            svg_icon(ICON_QUEUE_SVG, 16.0),
            text("Play Queue").size(14).width(Length::Fill),
            text(format!("{}", props.queued_track_ids.len())).size(11).style(Color::from_rgb8(148, 163, 184)),
        ]
        .spacing(8)
        .align_items(Alignment::Center),
    )
    .on_press(Message::SelectTab(NavTab::Queue))
    .width(Length::Fill)
    .padding([8, 12])
    .style(if is_queue_active { btn_selected_nav_style() } else { btn_nav_style() });

    let is_info_active = props.current_tab == NavTab::Info;
    let info_btn = button(
        row![
            svg_icon(ICON_INFO_SVG, 16.0),
            text("Audio & System").size(14).width(Length::Fill),
        ]
        .spacing(8)
        .align_items(Alignment::Center),
    )
    .on_press(Message::SelectTab(NavTab::Info))
    .width(Length::Fill)
    .padding([8, 12])
    .style(if is_info_active { btn_selected_nav_style() } else { btn_nav_style() });

    let mut folder_buttons = column![].spacing(4);
    for folder in folders {
        let label = folder
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Folder")
            .to_owned();
        let is_selected = props.selected_folder.map(|f| f == &folder).unwrap_or(false);

        folder_buttons = folder_buttons.push(
            button(
                row![
                    svg_icon(ICON_FOLDER_SVG, 14.0),
                    text(label).size(13).width(Length::Fill),
                ]
                .spacing(6)
                .align_items(Alignment::Center),
            )
            .on_press(Message::SelectFolder(Some(folder)))
            .width(Length::Fill)
            .padding([6, 10])
            .style(if is_selected { btn_selected_nav_style() } else { btn_nav_style() }),
        );
    }

    let sidebar_content = column![
        text("NAVIGATION").size(11).style(Color::from_rgb8(100, 116, 139)),
        lib_btn,
        queue_btn,
        info_btn,
        text("FOLDERS").size(11).style(Color::from_rgb8(100, 116, 139)),
        folder_buttons,
    ]
    .spacing(10);

    container(scrollable(sidebar_content).height(Length::Fill))
        .padding(14)
        .width(Length::Fixed(220.0))
        .height(Length::Fill)
        .style(panel_container_style())
        .into()
}

fn render_track_list<'a>(props: &ViewProps<'a>) -> Element<'a, Message> {
    if props.all_tracks.is_empty() {
        return container(
            column![
                svg_icon(LOGO_SVG, 64.0),
                text("Your Music Library is Empty").size(24).style(Color::WHITE),
                text("Add a music directory to index your songs, or generate sample audio tracks to test playback immediately.")
                    .size(14)
                    .style(Color::from_rgb8(148, 163, 184)),
                row![
                    button(
                        row![
                            svg_icon(ICON_PLAY_SVG, 14.0),
                            text("Generate Sample Audio Tracks").size(14).style(Color::WHITE),
                        ]
                        .spacing(8)
                        .align_items(Alignment::Center),
                    )
                    .on_press(Message::GenerateSampleAudio)
                    .padding([10, 18])
                    .style(btn_primary_style()),
                    button(
                        row![
                            svg_icon(ICON_FOLDER_SVG, 15.0),
                            text("Browse Music Folder").size(14),
                        ]
                        .spacing(8)
                        .align_items(Alignment::Center),
                    )
                    .on_press(Message::ImportFolder)
                    .padding([10, 18])
                    .style(btn_default_style()),
                ]
                .spacing(12),
            ]
            .spacing(14)
            .align_items(Alignment::Center),
        )
        .padding(40)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(panel_container_style())
        .into();
    }

    let header_title = if let Some(folder) = props.selected_folder {
        folder
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Music Folder")
            .to_string()
    } else {
        "All Tracks".to_string()
    };

    let title_row = row![
        text(header_title).size(20).style(Color::WHITE),
        horizontal_space(),
        text(format!("{} songs found", props.tracks.len()))
            .size(13)
            .style(Color::from_rgb8(148, 163, 184)),
    ]
    .align_items(Alignment::Center);

    // Column Headers
    let col_headers = row![
        text("#").width(Length::Fixed(36.0)).style(Color::from_rgb8(100, 116, 139)),
        text("TITLE").width(Length::FillPortion(4)).style(Color::from_rgb8(100, 116, 139)),
        text("ARTIST").width(Length::FillPortion(3)).style(Color::from_rgb8(100, 116, 139)),
        text("ALBUM").width(Length::FillPortion(3)).style(Color::from_rgb8(100, 116, 139)),
        text("TIME").width(Length::Fixed(60.0)).style(Color::from_rgb8(100, 116, 139)),
        text("ACTIONS").width(Length::Fixed(120.0)).style(Color::from_rgb8(100, 116, 139)),
    ]
    .spacing(8)
    .padding([6, 10]);

    let mut track_rows = column![].spacing(3);
    for (idx, track) in props.tracks.iter().enumerate() {
        let is_playing = props.current_playing_track_id == Some(track.id);
        let is_selected = props.selected_track == Some(track.id);
        let duration = track.duration.map(format_duration).unwrap_or_else(|| "--:--".into());

        let icon: Element<'a, Message> = if is_playing {
            container(svg_icon(ICON_PLAY_SVG, 12.0))
                .width(Length::Fixed(28.0))
                .into()
        } else {
            container(text(format!("{:02}", idx + 1)).size(13).style(Color::from_rgb8(100, 116, 139)))
                .width(Length::Fixed(28.0))
                .into()
        };

        let title_style = if is_playing {
            Color::from_rgb8(52, 211, 153)
        } else if is_selected {
            Color::from_rgb8(96, 165, 250)
        } else {
            Color::WHITE
        };

        let row_btn = button(
            row![
                icon,
                text(&track.title).width(Length::FillPortion(4)).size(14).style(title_style),
                text(&track.artist).width(Length::FillPortion(3)).size(13).style(Color::from_rgb8(148, 163, 184)),
                text(&track.album).width(Length::FillPortion(3)).size(13).style(Color::from_rgb8(148, 163, 184)),
                text(duration).width(Length::Fixed(56.0)).size(13).style(Color::from_rgb8(148, 163, 184)),
                row![
                    button(svg_icon(ICON_PLAY_SVG, 10.0))
                        .on_press(Message::PlayTrack(track.id))
                        .padding([5, 8])
                        .style(btn_play_row_style()),
                    button(text("+Q").size(11))
                        .on_press(Message::QueueTrack(track.id))
                        .padding([4, 6])
                        .style(btn_queue_row_style()),
                    button(svg_icon(ICON_EDIT_SVG, 11.0))
                        .on_press(Message::StartEditTrack(track.id))
                        .padding([4, 6])
                        .style(btn_default_style()),
                ]
                .spacing(4)
                .width(Length::Fixed(116.0)),
            ]
            .spacing(8)
            .align_items(Alignment::Center),
        )
        .on_press(Message::PlayTrack(track.id))
        .width(Length::Fill)
        .padding([7, 10])
        .style(if is_playing {
            btn_playing_row_style()
        } else if is_selected {
            btn_selected_row_style()
        } else {
            btn_item_row_style()
        });

        track_rows = track_rows.push(row_btn);
    }

    let mut content = column![title_row].spacing(8);
    if let Some(editing) = props.editing_track {
        content = content.push(render_tag_editor(editing));
    }
    content = content.push(col_headers).push(scrollable(track_rows).height(Length::Fill));

    container(content)
        .padding(14)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(panel_container_style())
        .into()
}

fn render_tag_editor<'a>(editing: &'a EditingTrackState) -> Element<'a, Message> {
    let title_input = text_input("Track Title", &editing.title)
        .on_input(|s| Message::EditFieldChanged(EditField::Title, s))
        .padding(8);
    let artist_input = text_input("Artist Name", &editing.artist)
        .on_input(|s| Message::EditFieldChanged(EditField::Artist, s))
        .padding(8);
    let album_input = text_input("Album Name", &editing.album)
        .on_input(|s| Message::EditFieldChanged(EditField::Album, s))
        .padding(8);
    let genre_input = text_input("Genre", &editing.genre)
        .on_input(|s| Message::EditFieldChanged(EditField::Genre, s))
        .padding(8);
    let year_input = text_input("Year", &editing.year)
        .on_input(|s| Message::EditFieldChanged(EditField::Year, s))
        .padding(8)
        .width(Length::Fixed(80.0));

    let save_btn = button(
        row![
            svg_icon(ICON_EDIT_SVG, 13.0),
            text("Save Tags").size(13).style(Color::WHITE),
        ]
        .spacing(6)
        .align_items(Alignment::Center),
    )
    .on_press(Message::SaveTrackTags)
    .padding([8, 16])
    .style(btn_primary_style());

    let cancel_btn = button(text("Cancel").size(13))
        .on_press(Message::CancelEditTrack)
        .padding([8, 14])
        .style(btn_default_style());

    container(
        column![
            row![
                svg_icon(ICON_EDIT_SVG, 16.0),
                text("Mass Tag Engine — Edit Track Metadata").size(15).style(Color::WHITE),
            ]
            .spacing(8)
            .align_items(Alignment::Center),
            row![
                column![text("Title").size(11).style(Color::from_rgb8(148, 163, 184)), title_input].spacing(4).width(Length::FillPortion(3)),
                column![text("Artist").size(11).style(Color::from_rgb8(148, 163, 184)), artist_input].spacing(4).width(Length::FillPortion(2)),
                column![text("Album").size(11).style(Color::from_rgb8(148, 163, 184)), album_input].spacing(4).width(Length::FillPortion(2)),
                column![text("Genre").size(11).style(Color::from_rgb8(148, 163, 184)), genre_input].spacing(4).width(Length::FillPortion(2)),
                column![text("Year").size(11).style(Color::from_rgb8(148, 163, 184)), year_input].spacing(4).width(Length::Fixed(80.0)),
            ]
            .spacing(10),
            row![save_btn, cancel_btn].spacing(8),
        ]
        .spacing(12),
    )
    .padding(14)
    .width(Length::Fill)
    .style(card_container_style())
    .into()
}

fn render_queue_list<'a>(props: &ViewProps<'a>) -> Element<'a, Message> {
    let header = row![
        row![
            svg_icon(ICON_QUEUE_SVG, 20.0),
            text("Current Playback Queue").size(20).style(Color::WHITE),
        ]
        .spacing(8)
        .align_items(Alignment::Center),
        horizontal_space(),
        button(text("Clear Queue").size(12))
            .on_press(Message::ClearQueue)
            .padding([6, 12])
            .style(btn_default_style()),
    ]
    .align_items(Alignment::Center);

    if props.queued_track_ids.is_empty() {
        return container(
            column![
                header,
                column![
                    text("Queue is empty").size(18).style(Color::WHITE),
                    text("Add songs from the library by clicking '+Q' to queue them up.")
                        .size(13)
                        .style(Color::from_rgb8(148, 163, 184)),
                ]
                .spacing(8)
                .align_items(Alignment::Center),
            ]
            .spacing(20),
        )
        .padding(14)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(panel_container_style())
        .into();
    }

    let mut rows = column![].spacing(4);
    for (q_idx, &track_id) in props.queued_track_ids.iter().enumerate() {
        let track = props.all_tracks.iter().find(|t| t.id == track_id);
        let title = track.map(|t| t.title.as_str()).unwrap_or("Unknown Track");
        let artist = track.map(|t| t.artist.as_str()).unwrap_or("Unknown Artist");
        let dur = track.and_then(|t| t.duration).map(format_duration).unwrap_or_else(|| "--:--".into());

        let row_item = container(
            row![
                text(format!("#{:02}", q_idx + 1)).size(13).style(Color::from_rgb8(100, 116, 139)).width(Length::Fixed(36.0)),
                text(title).size(14).style(Color::WHITE).width(Length::FillPortion(4)),
                text(artist).size(13).style(Color::from_rgb8(148, 163, 184)).width(Length::FillPortion(3)),
                text(dur).size(13).style(Color::from_rgb8(148, 163, 184)).width(Length::Fixed(60.0)),
                button(
                    row![
                        svg_icon(ICON_PLAY_SVG, 10.0),
                        text("Play").size(11).style(Color::WHITE),
                    ]
                    .spacing(4)
                    .align_items(Alignment::Center),
                )
                .on_press(Message::PlayTrack(track_id))
                .padding([4, 8])
                .style(btn_primary_style()),
                button(text("✕").size(11))
                    .on_press(Message::RemoveFromQueue(q_idx))
                    .padding([4, 8])
                    .style(btn_default_style()),
            ]
            .spacing(8)
            .align_items(Alignment::Center),
        )
        .padding([8, 12])
        .width(Length::Fill)
        .style(card_container_style());

        rows = rows.push(row_item);
    }

    container(
        column![header, scrollable(rows).height(Length::Fill)].spacing(12),
    )
    .padding(14)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(panel_container_style())
    .into()
}

fn render_info_view<'a>(props: &ViewProps<'a>) -> Element<'a, Message> {
    let title_row = row![
        svg_icon(ICON_INFO_SVG, 20.0),
        text("Audio Engine & System Information").size(22).style(Color::WHITE),
    ]
    .spacing(8)
    .align_items(Alignment::Center);

    // Audio Engine & DSP Specs
    let rg_status = if props.replaygain_enabled { "Enabled (-18 LUFS)" } else { "Disabled" };
    let gapless_status = if props.gapless_enabled { "Enabled (3.5s Lookahead)" } else { "Disabled" };

    let dsp_specs = column![
        text("Audio Processing Engine & DSP").size(16).style(Color::from_rgb8(52, 211, 153)),
        row![
            text("• Real-Time Audio Visualizer:").size(13).style(Color::from_rgb8(203, 213, 225)),
            text("28-Band Spectrum streamed at 30 Hz PCM").size(13).style(Color::from_rgb8(148, 163, 184)),
        ].spacing(6),
        row![
            text("• Loudness Normalization (EBU R128):").size(13).style(Color::from_rgb8(203, 213, 225)),
            text(rg_status).size(13).style(if props.replaygain_enabled { Color::from_rgb8(52, 211, 153) } else { Color::from_rgb8(148, 163, 184) }),
            button(text("Analyze Current / Selected Track Loudness").size(11))
                .on_press(Message::AnalyzeReplayGain(None))
                .padding([4, 8])
                .style(btn_accent_style()),
        ].spacing(8).align_items(Alignment::Center),
        row![
            text("• Gapless Audio Transitions:").size(13).style(Color::from_rgb8(203, 213, 225)),
            text(gapless_status).size(13).style(if props.gapless_enabled { Color::from_rgb8(52, 211, 153) } else { Color::from_rgb8(148, 163, 184) }),
        ].spacing(6),
        text("• High-resolution pipeline via CPAL (PipeWire, ALSA, or PulseAudio)").size(13).style(Color::from_rgb8(203, 213, 225)),
        text("• Sample-accurate position tracking with lock-free atomic clocks").size(13).style(Color::from_rgb8(203, 213, 225)),
        text("• Real-time logarithmic volume attenuation & instantaneous seeking").size(13).style(Color::from_rgb8(203, 213, 225)),
    ].spacing(8);

    // Multi-format decoding specs
    let format_specs = column![
        text("Supported Multi-Format Audio Codecs (Symphonia)").size(16).style(Color::from_rgb8(52, 211, 153)),
        text("• WebM / Matroska: .webm, .mkv (Opus, Vorbis, PCM audio)").size(13).style(Color::from_rgb8(203, 213, 225)),
        text("• MPEG Audio: .mp3, .mp2, .mp1 (Layer I, II, III)").size(13).style(Color::from_rgb8(203, 213, 225)),
        text("• Free Lossless Audio Codec: .flac (Native 16/24-bit lossless)").size(13).style(Color::from_rgb8(203, 213, 225)),
        text("• Waveform Audio: .wav, .wave (Linear PCM, IEEE float)").size(13).style(Color::from_rgb8(203, 213, 225)),
        text("• Ogg Bitstream: .ogg, .oga (Vorbis, Opus, FLAC)").size(13).style(Color::from_rgb8(203, 213, 225)),
        text("• MP4 / Advanced Audio: .m4a, .m4b, .mp4, .aac (AAC, ALAC)").size(13).style(Color::from_rgb8(203, 213, 225)),
        text("• Apple & Interchange: .aiff, .aif, .caf (AIFF, Core Audio Format)").size(13).style(Color::from_rgb8(203, 213, 225)),
    ].spacing(6);

    // Dynamic native components / plugins
    let mut comp_rows = column![].spacing(6);
    if props.components.is_empty() {
        comp_rows = comp_rows.push(
            text("No native components loaded. Place compiled .so dynamic libraries in the components/ folder.")
                .size(13)
                .style(Color::from_rgb8(148, 163, 184)),
        );
    } else {
        for comp in props.components {
            let row_card = container(
                row![
                    column![
                        text(comp.metadata.name).size(14).style(Color::WHITE),
                        text(format!("ID: {} • Version: {}", comp.metadata.id, comp.metadata.version))
                            .size(12)
                            .style(Color::from_rgb8(148, 163, 184)),
                    ].spacing(2).width(Length::Fill),
                    container(text("ACTIVE").size(11).style(Color::from_rgb8(52, 211, 153)))
                        .padding([4, 8])
                        .style(badge_container_style()),
                ]
                .align_items(Alignment::Center),
            )
            .padding([8, 12])
            .width(Length::Fill)
            .style(card_container_style());

            comp_rows = comp_rows.push(row_card);
        }
    }

    let components_section = column![
        text(format!("Native Dynamic Components ({})", props.components.len())).size(16).style(Color::from_rgb8(52, 211, 153)),
        comp_rows,
    ].spacing(8);

    let shortcuts_section = column![
        text("Shortcuts & Controls").size(16).style(Color::from_rgb8(52, 211, 153)),
        text("• Click any track to immediately start playback").size(13).style(Color::from_rgb8(203, 213, 225)),
        text("• Click '+Q' to add songs to the playback queue").size(13).style(Color::from_rgb8(203, 213, 225)),
        text("• Click the pencil icon to edit track metadata with the Mass Tagging engine").size(13).style(Color::from_rgb8(203, 213, 225)),
        text("• Drag the progress slider to scrub or seek anywhere in the track").size(13).style(Color::from_rgb8(203, 213, 225)),
        text("• Toggle RG (ReplayGain) or GAPLESS in the transport bar").size(13).style(Color::from_rgb8(203, 213, 225)),
    ].spacing(6);

    let scrollable_content = scrollable(
        column![
            title_row,
            dsp_specs,
            format_specs,
            components_section,
            shortcuts_section,
        ]
        .spacing(20),
    )
    .height(Length::Fill);

    container(scrollable_content)
        .padding(24)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(panel_container_style())
        .into()
}

fn render_visualizer<'a>(pcm: &'a [f32], is_playing: bool) -> Element<'a, Message> {
    const NUM_BARS: usize = 28;
    const MAX_HEIGHT: f32 = 22.0;
    const MIN_HEIGHT: f32 = 3.0;

    let mut bars = row![].spacing(3).align_items(Alignment::End);

    if !is_playing || pcm.is_empty() {
        for _ in 0..NUM_BARS {
            let bar = container(horizontal_space())
                .width(Length::Fixed(4.0))
                .height(Length::Fixed(MIN_HEIGHT))
                .style(visualizer_bar_style(0.0));
            bars = bars.push(bar);
        }
    } else {
        let chunk_size = (pcm.len() / NUM_BARS).max(1);
        for i in 0..NUM_BARS {
            let start = i * chunk_size;
            let end = (start + chunk_size).min(pcm.len());
            let chunk = &pcm[start..end];
            let mut sum_sq = 0.0_f32;
            for &s in chunk {
                sum_sq += s * s;
            }
            let rms = (sum_sq / chunk.len().max(1) as f32).sqrt();
            let norm = (rms * 3.2).clamp(0.0, 1.0);
            let height = MIN_HEIGHT + norm * (MAX_HEIGHT - MIN_HEIGHT);

            let bar = container(horizontal_space())
                .width(Length::Fixed(4.0))
                .height(Length::Fixed(height))
                .style(visualizer_bar_style(norm));
            bars = bars.push(bar);
        }
    }

    container(bars)
        .padding([2, 4])
        .height(Length::Fixed(MAX_HEIGHT + 4.0))
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Bottom)
        .into()
}

fn render_transport_bar<'a>(props: &ViewProps<'a>) -> Element<'a, Message> {
    // Left: Now Playing Metadata
    let track_title = if props.metadata.title.is_empty() {
        "Nothing Playing"
    } else {
        props.metadata.title.as_str()
    };
    let track_artist = if props.metadata.artist.is_empty() {
        "Kanono Media Player"
    } else {
        props.metadata.artist.as_str()
    };

    let mut details_col = column![
        text(track_title).size(14).style(Color::WHITE),
        text(track_artist).size(12).style(Color::from_rgb8(148, 163, 184)),
    ]
    .spacing(2);

    if let Some(gain) = props.current_track_gain {
        let gain_sign = if gain.gain_db >= 0.0 { "+" } else { "" };
        let gain_info = format!("RG: {}{:.1} dB ({:.1} LUFS)", gain_sign, gain.gain_db, gain.integrated_lufs);
        details_col = details_col.push(
            text(gain_info).size(10).style(Color::from_rgb8(52, 211, 153)),
        );
    }

    let now_playing_info = row![
        container(svg_icon(ICON_DISC_SVG, 22.0))
            .padding([8, 10])
            .style(badge_container_style()),
        details_col,
    ]
    .spacing(12)
    .align_items(Alignment::Center)
    .width(Length::Fixed(240.0));

    // Center: Playback Buttons + Seek Slider
    let prev_btn = button(
        row![
            svg_icon(ICON_PREV_SVG, 13.0),
            text("Prev").size(12),
        ]
        .spacing(4)
        .align_items(Alignment::Center),
    )
    .on_press(Message::Previous)
    .padding([6, 12])
    .style(btn_default_style());

    let (play_pause_svg, play_pause_label) = if props.is_playing {
        (ICON_PAUSE_SVG, "Pause")
    } else {
        (ICON_PLAY_SVG, "Play")
    };
    let play_btn = button(
        row![
            svg_icon(play_pause_svg, 14.0),
            text(play_pause_label).size(13).style(Color::WHITE),
        ]
        .spacing(6)
        .align_items(Alignment::Center),
    )
    .on_press(Message::TogglePlayback)
    .padding([8, 18])
    .style(btn_primary_style());

    let next_btn = button(
        row![
            text("Next").size(12),
            svg_icon(ICON_NEXT_SVG, 13.0),
        ]
        .spacing(4)
        .align_items(Alignment::Center),
    )
    .on_press(Message::Next)
    .padding([6, 12])
    .style(btn_default_style());

    let shuffle_style = if props.is_shuffled { btn_active_toggle_style() } else { btn_default_style() };
    let shuffle_btn = button(
        row![
            svg_icon(ICON_SHUFFLE_SVG, 13.0),
            text("Shuffle").size(11),
        ]
        .spacing(4)
        .align_items(Alignment::Center),
    )
    .on_press(Message::ToggleShuffle)
    .padding([6, 10])
    .style(shuffle_style);

    let repeat_style = if props.is_repeated { btn_active_toggle_style() } else { btn_default_style() };
    let repeat_btn = button(
        row![
            svg_icon(ICON_REPEAT_SVG, 13.0),
            text("Repeat").size(11),
        ]
        .spacing(4)
        .align_items(Alignment::Center),
    )
    .on_press(Message::ToggleRepeat)
    .padding([6, 10])
    .style(repeat_style);

    let rg_style = if props.replaygain_enabled { btn_active_toggle_style() } else { btn_default_style() };
    let rg_btn = button(text("RG").size(11))
        .on_press(Message::ToggleReplayGain)
        .padding([6, 8])
        .style(rg_style);

    let gapless_style = if props.gapless_enabled { btn_active_toggle_style() } else { btn_default_style() };
    let gapless_btn = button(text("GAPLESS").size(10))
        .on_press(Message::ToggleGapless)
        .padding([6, 8])
        .style(gapless_style);

    let controls_row = row![shuffle_btn, prev_btn, play_btn, next_btn, repeat_btn, rg_btn, gapless_btn]
        .spacing(8)
        .align_items(Alignment::Center);

    let total_secs = props.metadata.duration.map(|d| d.as_secs_f32()).unwrap_or(0.0);
    let elapsed_secs = props.elapsed.as_secs_f32();
    let seek_fraction = if total_secs > 0.0 {
        (elapsed_secs / total_secs).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let elapsed_str = format_duration(props.elapsed);
    let total_str = props.metadata.duration.map(format_duration).unwrap_or_else(|| "--:--".into());

    let seek_slider = slider(0.0..=1.0, seek_fraction, Message::Seek)
        .step(0.005_f32)
        .width(Length::Fill);

    let scrub_row = row![
        text(elapsed_str).size(12).style(Color::from_rgb8(148, 163, 184)),
        seek_slider,
        text(total_str).size(12).style(Color::from_rgb8(148, 163, 184)),
    ]
    .spacing(10)
    .align_items(Alignment::Center)
    .width(Length::Fill);

    let visualizer = render_visualizer(props.visualizer_pcm, props.is_playing);

    let center_controls = column![visualizer, controls_row, scrub_row]
        .spacing(4)
        .align_items(Alignment::Center)
        .width(Length::Fill);

    // Right: Volume & Output Info
    let mute_svg = if props.is_muted || props.volume == 0.0 {
        ICON_MUTE_SVG
    } else {
        ICON_VOLUME_SVG
    };
    let mute_btn = button(svg_icon(mute_svg, 16.0))
        .on_press(Message::ToggleMute)
        .padding([6, 8])
        .style(btn_default_style());

    let vol_slider = slider(0.0..=1.0, props.volume, Message::VolumeChanged)
        .step(0.02_f32)
        .width(Length::Fixed(90.0));

    let vol_percent = format!("{:.0}%", props.volume * 100.0);

    let volume_controls = row![
        mute_btn,
        vol_slider,
        text(vol_percent).size(11).style(Color::from_rgb8(148, 163, 184)).width(Length::Fixed(34.0)),
    ]
    .spacing(6)
    .align_items(Alignment::Center)
    .width(Length::Fixed(180.0));

    container(
        row![now_playing_info, center_controls, volume_controls]
            .spacing(16)
            .align_items(Alignment::Center),
    )
    .padding([8, 16])
    .width(Length::Fill)
    .style(panel_container_style())
    .into()
}

fn folder_roots(tracks: &[LibraryTrack]) -> Vec<PathBuf> {
    tracks
        .iter()
        .filter_map(|track| track.path.parent().map(PathBuf::from))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn format_duration(duration: Duration) -> String {
    format!("{:02}:{:02}", duration.as_secs() / 60, duration.as_secs() % 60)
}

// ==========================================
// STYLING IMPLEMENTATIONS (ICED 0.12)
// ==========================================

fn root_background_style() -> iced::theme::Container {
    iced::theme::Container::Custom(Box::new(RootBgStyle))
}

fn panel_container_style() -> iced::theme::Container {
    iced::theme::Container::Custom(Box::new(PanelStyle))
}

fn card_container_style() -> iced::theme::Container {
    iced::theme::Container::Custom(Box::new(CardStyle))
}

fn badge_container_style() -> iced::theme::Container {
    iced::theme::Container::Custom(Box::new(BadgeStyle))
}

fn btn_primary_style() -> iced::theme::Button {
    iced::theme::Button::Custom(Box::new(PrimaryButtonStyle))
}

fn btn_default_style() -> iced::theme::Button {
    iced::theme::Button::Custom(Box::new(DefaultButtonStyle))
}

fn btn_accent_style() -> iced::theme::Button {
    iced::theme::Button::Custom(Box::new(AccentButtonStyle))
}

fn btn_nav_style() -> iced::theme::Button {
    iced::theme::Button::Custom(Box::new(NavButtonStyle))
}

fn btn_selected_nav_style() -> iced::theme::Button {
    iced::theme::Button::Custom(Box::new(SelectedNavButtonStyle))
}

fn btn_item_row_style() -> iced::theme::Button {
    iced::theme::Button::Custom(Box::new(ItemRowButtonStyle))
}

fn btn_selected_row_style() -> iced::theme::Button {
    iced::theme::Button::Custom(Box::new(SelectedRowButtonStyle))
}

fn btn_playing_row_style() -> iced::theme::Button {
    iced::theme::Button::Custom(Box::new(PlayingRowButtonStyle))
}

fn btn_play_row_style() -> iced::theme::Button {
    iced::theme::Button::Custom(Box::new(PlayRowButtonStyle))
}

fn btn_queue_row_style() -> iced::theme::Button {
    iced::theme::Button::Custom(Box::new(QueueRowButtonStyle))
}

fn btn_active_toggle_style() -> iced::theme::Button {
    iced::theme::Button::Custom(Box::new(ActiveToggleButtonStyle))
}

// Containers
struct RootBgStyle;
impl iced::widget::container::StyleSheet for RootBgStyle {
    type Style = iced::Theme;
    fn appearance(&self, _style: &Self::Style) -> iced::widget::container::Appearance {
        iced::widget::container::Appearance {
            background: Some(Color::from_rgb8(11, 14, 20).into()),
            text_color: Some(Color::WHITE),
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
        }
    }
}

struct PanelStyle;
impl iced::widget::container::StyleSheet for PanelStyle {
    type Style = iced::Theme;
    fn appearance(&self, _style: &Self::Style) -> iced::widget::container::Appearance {
        iced::widget::container::Appearance {
            background: Some(Color::from_rgb8(18, 22, 32).into()),
            text_color: Some(Color::WHITE),
            border: iced::Border {
                color: Color::from_rgb8(33, 40, 56),
                width: 1.0,
                radius: 12.0.into(),
            },
            shadow: iced::Shadow::default(),
        }
    }
}

struct CardStyle;
impl iced::widget::container::StyleSheet for CardStyle {
    type Style = iced::Theme;
    fn appearance(&self, _style: &Self::Style) -> iced::widget::container::Appearance {
        iced::widget::container::Appearance {
            background: Some(Color::from_rgb8(24, 30, 44).into()),
            text_color: Some(Color::WHITE),
            border: iced::Border {
                color: Color::from_rgb8(42, 52, 72),
                width: 1.0,
                radius: 8.0.into(),
            },
            shadow: iced::Shadow::default(),
        }
    }
}

struct BadgeStyle;
impl iced::widget::container::StyleSheet for BadgeStyle {
    type Style = iced::Theme;
    fn appearance(&self, _style: &Self::Style) -> iced::widget::container::Appearance {
        iced::widget::container::Appearance {
            background: Some(Color::from_rgb8(30, 41, 59).into()),
            text_color: Some(Color::WHITE),
            border: iced::Border {
                color: Color::from_rgb8(51, 65, 85),
                width: 1.0,
                radius: 8.0.into(),
            },
            shadow: iced::Shadow::default(),
        }
    }
}

// Buttons
struct PrimaryButtonStyle;
impl iced::widget::button::StyleSheet for PrimaryButtonStyle {
    type Style = iced::Theme;
    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(Color::from_rgb8(16, 185, 129).into()),
            text_color: Color::WHITE,
            border: iced::Border {
                color: Color::from_rgb8(52, 211, 153),
                width: 1.0,
                radius: 8.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
    fn hovered(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(Color::from_rgb8(5, 150, 105).into()),
            text_color: Color::WHITE,
            border: iced::Border {
                color: Color::from_rgb8(110, 231, 183),
                width: 1.0,
                radius: 8.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
}

struct DefaultButtonStyle;
impl iced::widget::button::StyleSheet for DefaultButtonStyle {
    type Style = iced::Theme;
    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(Color::from_rgb8(28, 35, 49).into()),
            text_color: Color::WHITE,
            border: iced::Border {
                color: Color::from_rgb8(45, 56, 78),
                width: 1.0,
                radius: 8.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
    fn hovered(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(Color::from_rgb8(40, 50, 70).into()),
            text_color: Color::WHITE,
            border: iced::Border {
                color: Color::from_rgb8(71, 85, 105),
                width: 1.0,
                radius: 8.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
}

struct AccentButtonStyle;
impl iced::widget::button::StyleSheet for AccentButtonStyle {
    type Style = iced::Theme;
    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(Color::from_rgb8(37, 99, 235).into()),
            text_color: Color::WHITE,
            border: iced::Border {
                color: Color::from_rgb8(96, 165, 250),
                width: 1.0,
                radius: 8.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
    fn hovered(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(Color::from_rgb8(29, 78, 216).into()),
            text_color: Color::WHITE,
            border: iced::Border {
                color: Color::from_rgb8(147, 197, 253),
                width: 1.0,
                radius: 8.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
}

struct NavButtonStyle;
impl iced::widget::button::StyleSheet for NavButtonStyle {
    type Style = iced::Theme;
    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(Color::from_rgb8(20, 25, 36).into()),
            text_color: Color::from_rgb8(226, 232, 240),
            border: iced::Border {
                color: Color::TRANSPARENT,
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
    fn hovered(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(Color::from_rgb8(30, 38, 55).into()),
            text_color: Color::WHITE,
            border: iced::Border {
                color: Color::from_rgb8(51, 65, 85),
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
}

struct SelectedNavButtonStyle;
impl iced::widget::button::StyleSheet for SelectedNavButtonStyle {
    type Style = iced::Theme;
    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(Color::from_rgb8(30, 58, 138).into()),
            text_color: Color::WHITE,
            border: iced::Border {
                color: Color::from_rgb8(59, 130, 246),
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
}

struct ItemRowButtonStyle;
impl iced::widget::button::StyleSheet for ItemRowButtonStyle {
    type Style = iced::Theme;
    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(Color::from_rgb8(16, 20, 29).into()),
            text_color: Color::WHITE,
            border: iced::Border {
                color: Color::from_rgb8(26, 32, 46),
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
    fn hovered(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(Color::from_rgb8(26, 34, 49).into()),
            text_color: Color::WHITE,
            border: iced::Border {
                color: Color::from_rgb8(51, 65, 85),
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
}

struct SelectedRowButtonStyle;
impl iced::widget::button::StyleSheet for SelectedRowButtonStyle {
    type Style = iced::Theme;
    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(Color::from_rgb8(30, 41, 69).into()),
            text_color: Color::WHITE,
            border: iced::Border {
                color: Color::from_rgb8(59, 130, 246),
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
}

struct PlayingRowButtonStyle;
impl iced::widget::button::StyleSheet for PlayingRowButtonStyle {
    type Style = iced::Theme;
    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(Color::from_rgb8(6, 78, 59).into()),
            text_color: Color::WHITE,
            border: iced::Border {
                color: Color::from_rgb8(16, 185, 129),
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
    fn hovered(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(Color::from_rgb8(4, 120, 87).into()),
            text_color: Color::WHITE,
            border: iced::Border {
                color: Color::from_rgb8(52, 211, 153),
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
}

struct PlayRowButtonStyle;
impl iced::widget::button::StyleSheet for PlayRowButtonStyle {
    type Style = iced::Theme;
    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(Color::from_rgb8(16, 185, 129).into()),
            text_color: Color::WHITE,
            border: iced::Border {
                color: Color::from_rgb8(52, 211, 153),
                width: 1.0,
                radius: 4.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
}

struct QueueRowButtonStyle;
impl iced::widget::button::StyleSheet for QueueRowButtonStyle {
    type Style = iced::Theme;
    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(Color::from_rgb8(51, 65, 85).into()),
            text_color: Color::WHITE,
            border: iced::Border {
                color: Color::from_rgb8(71, 85, 105),
                width: 1.0,
                radius: 4.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
}

struct ActiveToggleButtonStyle;
impl iced::widget::button::StyleSheet for ActiveToggleButtonStyle {
    type Style = iced::Theme;
    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(Color::from_rgb8(16, 185, 129).into()),
            text_color: Color::WHITE,
            border: iced::Border {
                color: Color::from_rgb8(52, 211, 153),
                width: 1.0,
                radius: 8.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
}

fn visualizer_bar_style(ratio: f32) -> iced::theme::Container {
    iced::theme::Container::Custom(Box::new(VisualizerBarStyle { ratio }))
}

struct VisualizerBarStyle {
    ratio: f32,
}

impl iced::widget::container::StyleSheet for VisualizerBarStyle {
    type Style = iced::Theme;
    fn appearance(&self, _style: &Self::Style) -> iced::widget::container::Appearance {
        let (r, g, b) = if self.ratio < 0.05 {
            (51, 65, 85)
        } else if self.ratio < 0.6 {
            (16, 185, 129)
        } else if self.ratio < 0.85 {
            (6, 182, 212)
        } else {
            (245, 158, 11)
        };

        iced::widget::container::Appearance {
            background: Some(Color::from_rgb8(r, g, b).into()),
            text_color: Some(Color::WHITE),
            border: iced::Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 2.0.into(),
            },
            shadow: iced::Shadow::default(),
        }
    }
}

