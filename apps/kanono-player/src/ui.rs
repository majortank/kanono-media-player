use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    time::Duration,
};

use iced::{
    widget::{button, column, container, horizontal_space, pane_grid, row, scrollable, slider, svg, text, text_input},
    Alignment, Color, Element, Length,
};
use kanono_audio_engine::{
    AudioDeviceInfo, LatencyProfile, LibraryFolder, LibraryStats, LibraryTrack, ReplayGainResult,
    SoundProfile, TargetLoudness, TrackMetadata,
};

use crate::{
    plugins::RegisteredPlugin,
    theme::{PlayerTheme, ThemePalette},
    BatchEditState, BatchField, BpmFilter, DurationFilter, EditField, EditingTrackState,
    FilterCategory, FilterState, Message, NavTab,
};

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
const ICON_FILE_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#94a3b8"><path d="M14 2H6c-1.1 0-1.99.9-1.99 2L2 18c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8l-6-6zm2 16H8v-2h8v2zm0-4H8v-2h8v2zm-3-5V3.5L18.5 9H13z"/></svg>"##;
const ICON_FIRE_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#f59e0b"><path d="M12 23c-4.97 0-9-4.03-9-9 0-3.3 1.8-6.19 4.47-7.75.52-.3 1.15.08 1.15.68v.57c0 1.25.77 2.37 1.94 2.8 1.48.55 2.44 1.98 2.44 3.56 0 .55.45 1 1 1s1-.45 1-1c0-2.31-1.35-4.32-3.32-5.26-.64-.31-.83-1.12-.39-1.66C12.3 5.48 14.28 4.2 16.5 4.03c.59-.05 1.05.47.95 1.05-.33 1.95.42 3.93 1.97 5.17C20.47 11.1 21 12.5 21 14c0 4.97-4.03 9-9 9z"/></svg>"##;
const ICON_CHEVRON_RIGHT_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#94a3b8"><polygon points="8,5 16,12 8,19"/></svg>"##;
const ICON_CHEVRON_DOWN_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#94a3b8"><polygon points="5,8 12,16 19,8"/></svg>"##;
const ICON_CHECK_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#10b981"><polygon points="9,16.2 4.8,12 3.4,13.4 9,19 21,7 19.6,5.6"/></svg>"##;
const ICON_FILTER_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#94a3b8"><path d="M10 18h4v-2h-4v2zM3 6v2h18V6H3zm3 7h12v-2H6v2z"/></svg>"##;
const ICON_CLEAR_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#cbd5e1"><path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12 19 6.41z"/></svg>"##;
const ICON_MANAGER_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#94a3b8"><path d="M4 6H2v14c0 1.1.9 2 2 2h14v-2H4V6zm16-4H8c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V4c0-1.1-.9-2-2-2zm-1 9H9V9h10v2zm-4 4H9v-2h6v2zm4-8H9V5h10v2z"/></svg>"##;
const ICON_REFRESH_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#cbd5e1"><path d="M17.65 6.35C16.2 4.9 14.21 4 12 4c-4.42 0-7.99 3.58-7.99 8s3.57 8 7.99 8c3.73 0 6.84-2.55 7.73-6h-2.08c-.82 2.33-3.04 4-5.65 4-3.31 0-6-2.69-6-6s2.69-6 6-6c1.66 0 3.14.69 4.22 1.78L13 11h7V4l-2.35 2.35z"/></svg>"##;
const ICON_TRASH_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#ef4444"><path d="M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z"/></svg>"##;
const ICON_BROOM_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#38bdf8"><path d="M19.36 2.72l1.42 1.42-7.07 7.07 1.41 1.41-1.41 1.42-2.83-2.83 1.41-1.42 1.42 1.42 7.07-7.07zM5.5 17.5l4-4 1.42 1.42-4 4H5.5v-1.42zm-2 2v2h2l6.5-6.5-2-2L3.5 19.5z"/></svg>"##;
const ICON_PALETTE_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#cbd5e1"><path d="M12 3c-4.97 0-9 4.03-9 9 0 2.12.74 4.07 1.97 5.61.42.53 1.05.89 1.73.89h1.8c.83 0 1.5.67 1.5 1.5 0 .38-.14.73-.38 1-.29.33-.42.75-.42 1.2 0 1.1.9 2 2 2 4.97 0 9-4.03 9-9 0-7.18-5.82-13-13-13zm-5.5 9c-.83 0-1.5-.67-1.5-1.5s.67-1.5 1.5-1.5 1.5.67 1.5 1.5-.67 1.5-1.5 1.5zm3-4c-.83 0-1.5-.67-1.5-1.5s.67-1.5 1.5-1.5 1.5.67 1.5 1.5-.67 1.5-1.5 1.5zm5 0c-.83 0-1.5-.67-1.5-1.5s.67-1.5 1.5-1.5 1.5.67 1.5 1.5-.67 1.5-1.5 1.5zm3 4c-.83 0-1.5-.67-1.5-1.5s.67-1.5 1.5-1.5 1.5.67 1.5 1.5-.67 1.5-1.5 1.5z"/></svg>"##;
const ICON_SPEAKER_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#cbd5e1"><path d="M7 9v6h4l5 5V4L11 9H7z"/></svg>"##;
const ICON_EQUALIZER_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#cbd5e1"><path d="M10 20h4V4h-4v16zm-6 0h4v-8H4v8zM16 9v11h4V9h-4z"/></svg>"##;
const ICON_CHIP_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#cbd5e1"><path d="M6 4h12v16H6zM4 6H2v2h2V6zm0 5H2v2h2v-2zm0 5H2v2h2v-2zm18-10h-2v2h2V6zm0 5h-2v2h2v-2zm0 5h-2v2h2v-2zM8 2h2v2H8V2zm6 0h2v2h-2V2zm-6 20h2v-2H8v2zm6 0h2v-2h-2v2z"/></svg>"##;

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
    pub expanded_folders: &'a HashSet<PathBuf>,
    pub current_tab: NavTab,
    pub filter_state: &'a FilterState,
    pub search: &'a str,
    pub selected_track: Option<i64>,
    pub selected_track_ids: &'a HashSet<i64>,
    pub batch_edit: &'a BatchEditState,
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
    pub seeking_fraction: Option<f32>,
    pub status_message: Option<&'a str>,
    pub visualizer_pcm: &'a [f32],
    pub current_track_gain: Option<ReplayGainResult>,
    pub editing_track: Option<&'a EditingTrackState>,
    pub components: &'a [RegisteredPlugin],
    pub library_folders: &'a [LibraryFolder],
    pub library_stats: &'a LibraryStats,
    pub is_scanning: bool,
    pub confirm_clear_library: bool,
    pub folder_tree: &'a [FolderTreeNode],
    pub theme: PlayerTheme,
    pub palette: ThemePalette,
    pub sound_profile: SoundProfile,
    pub target_loudness: TargetLoudness,
    pub latency_profile: LatencyProfile,
    pub audio_device_info: &'a AudioDeviceInfo,
    pub inspecting_track: Option<&'a LibraryTrack>,
}

#[derive(Debug, Clone, Copy)]
pub enum PlayerPane {
    Sidebar,
    Main,
}

pub fn player_view<'a>(props: ViewProps<'a>, panes: &'a pane_grid::State<PlayerPane>) -> Element<'a, Message> {
    let p = &props.palette;

    // --- TOP BAR ---
    let logo_icon = svg(svg::Handle::from_memory(LOGO_SVG))
        .width(Length::Fixed(28.0))
        .height(Length::Fixed(28.0));

    let logo = row![
        logo_icon,
        text("KANONO").size(18).style(p.accent),
        text("PLAYER").size(18).style(p.text_primary),
    ]
    .spacing(8)
    .align_items(Alignment::Center);

    let mut search_row = row![
        text_input("Search music library by title, artist, album, genre...", props.search)
            .on_input(Message::SearchChanged)
            .padding(9)
            .width(Length::Fill)
    ]
    .spacing(6)
    .align_items(Alignment::Center)
    .width(Length::Fill);

    if !props.search.is_empty() {
        search_row = search_row.push(
            button(svg_icon(ICON_CLEAR_SVG, 12.0))
                .on_press(Message::SearchChanged(String::new()))
                .padding([8, 10])
                .style(btn_default_style(p)),
        );
    }

    let top_actions = row![
        button(
            row![
                svg_icon(ICON_FILE_SVG, 15.0),
                text("Add Files").size(13),
            ]
            .spacing(6)
            .align_items(Alignment::Center),
        )
        .on_press(Message::ImportFiles)
        .padding([8, 14])
        .style(btn_default_style(p)),

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
        .style(btn_default_style(p)),

        button(
            row![
                svg_icon(ICON_MANAGER_SVG, 15.0),
                text("Library Manager").size(13),
            ]
            .spacing(6)
            .align_items(Alignment::Center),
        )
        .on_press(Message::SelectTab(NavTab::LibraryManager))
        .padding([8, 14])
        .style(if props.current_tab == NavTab::LibraryManager { btn_selected_nav_style(p) } else { btn_default_style(p) }),

        button(
            row![
                svg_icon(ICON_PALETTE_SVG, 14.0),
                text(props.theme.label()).size(13).style(p.text_primary),
            ]
            .spacing(6)
            .align_items(Alignment::Center),
        )
        .on_press(Message::CycleTheme)
        .padding([8, 12])
        .style(btn_default_style(p)),

        button(
            row![
                svg_icon(ICON_PLAY_SVG, 13.0),
                text("Sample Audio").size(13).style(p.accent_text),
            ]
            .spacing(6)
            .align_items(Alignment::Center),
        )
        .on_press(Message::GenerateSampleAudio)
        .padding([8, 14])
        .style(btn_accent_style(p)),
    ]
    .spacing(8)
    .align_items(Alignment::Center);

    let mut top_row = row![logo, search_row].spacing(16).align_items(Alignment::Center);
    if let Some(status) = props.status_message {
        top_row = top_row.push(
            container(text(status).size(12).style(p.accent))
                .padding([6, 10])
                .style(badge_container_style(p)),
        );
    }
    top_row = top_row.push(top_actions);

    let top_bar = container(top_row)
        .padding([10, 16])
        .width(Length::Fill)
        .style(panel_container_style(p));

    let middle = pane_grid::PaneGrid::new(panes, |_, pane, _| {
        pane_grid::Content::new(match pane {
            PlayerPane::Sidebar => render_sidebar(&props),
            PlayerPane::Main => match props.current_tab {
                NavTab::Library | NavTab::Folders | NavTab::MostPlayed => render_track_list(&props),
                NavTab::Queue => render_queue_list(&props),
                NavTab::LibraryManager => render_library_manager(&props),
                NavTab::Info => render_info_view(&props),
            },
        })
    })
        .on_resize(12, Message::SidebarResized)
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
    .style(root_background_style(p))
    .into()
}

fn render_sidebar<'a>(props: &ViewProps<'a>) -> Element<'a, Message> {
    let p = &props.palette;

    let is_lib_active = props.current_tab == NavTab::Library
        && props.selected_folder.is_none()
        && !props.filter_state.is_any_active();
    let lib_btn = button(
        row![
            svg_icon(ICON_LIBRARY_SVG, 16.0),
            text("All Tracks").size(14).width(Length::Fill),
            text(format!("{}", props.all_tracks.len())).size(11).style(p.text_secondary),
        ]
        .spacing(8)
        .align_items(Alignment::Center),
    )
    .on_press(Message::SelectTab(NavTab::Library))
    .width(Length::Fill)
    .padding([8, 12])
    .style(if is_lib_active { btn_selected_nav_style(p) } else { btn_nav_style(p) });

    let most_played_count = props.all_tracks.iter().filter(|t| t.play_count > 0).count();
    let is_most_played_active = props.current_tab == NavTab::MostPlayed;
    let most_played_btn = button(
        row![
            svg_icon(ICON_FIRE_SVG, 16.0),
            text("Most Played").size(14).width(Length::Fill),
            text(format!("{}", most_played_count)).size(11).style(p.accent),
        ]
        .spacing(8)
        .align_items(Alignment::Center),
    )
    .on_press(Message::SelectTab(NavTab::MostPlayed))
    .width(Length::Fill)
    .padding([8, 12])
    .style(if is_most_played_active { btn_selected_nav_style(p) } else { btn_nav_style(p) });

    let is_queue_active = props.current_tab == NavTab::Queue;
    let queue_btn = button(
        row![
            svg_icon(ICON_QUEUE_SVG, 16.0),
            text("Play Queue").size(14).width(Length::Fill),
            text(format!("{}", props.queued_track_ids.len())).size(11).style(p.text_secondary),
        ]
        .spacing(8)
        .align_items(Alignment::Center),
    )
    .on_press(Message::SelectTab(NavTab::Queue))
    .width(Length::Fill)
    .padding([8, 12])
    .style(if is_queue_active { btn_selected_nav_style(p) } else { btn_nav_style(p) });

    let is_manager_active = props.current_tab == NavTab::LibraryManager;
    let manager_btn = button(
        row![
            svg_icon(ICON_MANAGER_SVG, 16.0),
            text("Library Manager").size(14).width(Length::Fill),
            text(format!("{} folders", props.library_folders.len())).size(11).style(p.text_secondary),
        ]
        .spacing(8)
        .align_items(Alignment::Center),
    )
    .on_press(Message::SelectTab(NavTab::LibraryManager))
    .width(Length::Fill)
    .padding([8, 12])
    .style(if is_manager_active { btn_selected_nav_style(p) } else { btn_nav_style(p) });

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
    .style(if is_info_active { btn_selected_nav_style(p) } else { btn_nav_style(p) });

    // Category Selector
    let cat_btn = |cat: FilterCategory, label: &'static str| -> Element<'a, Message> {
        let is_active = props.filter_state.active_category == cat;
        button(text(label).size(11))
            .on_press(Message::SelectFilterCategory(cat))
            .padding([4, 8])
            .style(btn_chip_style(is_active, p))
            .into()
    };

    let cat_chips_1 = row![
        cat_btn(FilterCategory::All, "All"),
        cat_btn(FilterCategory::Artists, "Artists"),
        cat_btn(FilterCategory::Albums, "Albums"),
        cat_btn(FilterCategory::Genres, "Genres"),
    ]
    .spacing(4);

    let cat_chips_2 = row![
        cat_btn(FilterCategory::Folders, "Folders"),
        cat_btn(FilterCategory::Duration, "Duration"),
        cat_btn(FilterCategory::Year, "Year"),
        cat_btn(FilterCategory::Bpm, "BPM"),
    ]
    .spacing(4);

    // Filter details / active options
    let filter_details: Element<'a, Message> = match props.filter_state.active_category {
        FilterCategory::Artists => {
            let mut artists_map: HashMap<String, usize> = HashMap::new();
            for t in props.all_tracks {
                if !t.artist.is_empty() {
                    *artists_map.entry(t.artist.clone()).or_insert(0) += 1;
                }
            }
            let mut artists: Vec<_> = artists_map.into_iter().collect();
            artists.sort_by_key(|a| a.0.to_lowercase());

            let mut artist_list = column![].spacing(3);
            for (artist, count) in artists {
                let is_sel = props.filter_state.artist.as_ref().map(|a| a == &artist).unwrap_or(false);
                let btn = button(
                    row![
                        text(&artist).size(12).width(Length::Fill),
                        text(format!("{}", count)).size(10).style(p.text_muted),
                    ]
                    .spacing(4)
                    .align_items(Alignment::Center),
                )
                .on_press(Message::SetArtistFilter(if is_sel { None } else { Some(artist) }))
                .padding([4, 8])
                .width(Length::Fill)
                .style(if is_sel { btn_selected_nav_style(p) } else { btn_invisible_style() });
                artist_list = artist_list.push(btn);
            }
            artist_list.into()
        }
        FilterCategory::Albums => {
            let mut albums_map: HashMap<String, usize> = HashMap::new();
            for t in props.all_tracks {
                if !t.album.is_empty() {
                    *albums_map.entry(t.album.clone()).or_insert(0) += 1;
                }
            }
            let mut albums: Vec<_> = albums_map.into_iter().collect();
            albums.sort_by_key(|a| a.0.to_lowercase());

            let mut album_list = column![].spacing(3);
            for (album, count) in albums {
                let is_sel = props.filter_state.album.as_ref().map(|a| a == &album).unwrap_or(false);
                let btn = button(
                    row![
                        text(&album).size(12).width(Length::Fill),
                        text(format!("{}", count)).size(10).style(p.text_muted),
                    ]
                    .spacing(4)
                    .align_items(Alignment::Center),
                )
                .on_press(Message::SetAlbumFilter(if is_sel { None } else { Some(album) }))
                .padding([4, 8])
                .width(Length::Fill)
                .style(if is_sel { btn_selected_nav_style(p) } else { btn_invisible_style() });
                album_list = album_list.push(btn);
            }
            album_list.into()
        }
        FilterCategory::Genres => {
            let mut genres_map: HashMap<String, usize> = HashMap::new();
            for t in props.all_tracks {
                if !t.genre.is_empty() {
                    *genres_map.entry(t.genre.clone()).or_insert(0) += 1;
                }
            }
            let mut genres: Vec<_> = genres_map.into_iter().collect();
            genres.sort_by_key(|a| a.0.to_lowercase());

            let mut genre_list = column![].spacing(3);
            for (genre, count) in genres {
                let is_sel = props.filter_state.genre.as_ref().map(|g| g == &genre).unwrap_or(false);
                let btn = button(
                    row![
                        text(&genre).size(12).width(Length::Fill),
                        text(format!("{}", count)).size(10).style(p.text_muted),
                    ]
                    .spacing(4)
                    .align_items(Alignment::Center),
                )
                .on_press(Message::SetGenreFilter(if is_sel { None } else { Some(genre) }))
                .padding([4, 8])
                .width(Length::Fill)
                .style(if is_sel { btn_selected_nav_style(p) } else { btn_invisible_style() });
                genre_list = genre_list.push(btn);
            }
            genre_list.into()
        }
        FilterCategory::Duration => {
            let dur_btn = |df: DurationFilter, label: &'static str| -> Element<'a, Message> {
                let is_sel = props.filter_state.duration == Some(df);
                button(text(label).size(12))
                    .on_press(Message::SetDurationFilter(if is_sel { None } else { Some(df) }))
                    .padding([5, 10])
                    .width(Length::Fill)
                    .style(if is_sel { btn_selected_nav_style(p) } else { btn_default_style(p) })
                    .into()
            };
            column![
                dur_btn(DurationFilter::Short, "< 2 minutes"),
                dur_btn(DurationFilter::Medium, "2 - 4 minutes"),
                dur_btn(DurationFilter::Long, "4 - 6 minutes"),
                dur_btn(DurationFilter::ExtraLong, "> 6 minutes"),
            ]
            .spacing(4)
            .into()
        }
        FilterCategory::Year => {
            let mut years_map: HashMap<i32, usize> = HashMap::new();
            for t in props.all_tracks {
                if let Some(y) = t.year {
                    *years_map.entry(y).or_insert(0) += 1;
                }
            }
            let mut years: Vec<_> = years_map.into_iter().collect();
            years.sort_by_key(|a| std::cmp::Reverse(a.0));

            if years.is_empty() {
                column![text("No year metadata found").size(12).style(p.text_muted)].into()
            } else {
                let mut year_list = column![].spacing(3);
                for (year, count) in years {
                    let is_sel = props.filter_state.year == Some(year);
                    let btn = button(
                        row![
                            text(format!("{}", year)).size(12).width(Length::Fill),
                            text(format!("{}", count)).size(10).style(p.text_muted),
                        ]
                        .spacing(4)
                        .align_items(Alignment::Center),
                    )
                    .on_press(Message::SetYearFilter(if is_sel { None } else { Some(year) }))
                    .padding([4, 8])
                    .width(Length::Fill)
                    .style(if is_sel { btn_selected_nav_style(p) } else { btn_invisible_style() });
                    year_list = year_list.push(btn);
                }
                year_list.into()
            }
        }
        FilterCategory::Bpm => {
            let bpm_btn = |bf: BpmFilter, label: &'static str| -> Element<'a, Message> {
                let is_sel = props.filter_state.bpm == Some(bf);
                button(text(label).size(12))
                    .on_press(Message::SetBpmFilter(if is_sel { None } else { Some(bf) }))
                    .padding([5, 10])
                    .width(Length::Fill)
                    .style(if is_sel { btn_selected_nav_style(p) } else { btn_default_style(p) })
                    .into()
            };
            column![
                bpm_btn(BpmFilter::Slow, "< 90 BPM (Chill)"),
                bpm_btn(BpmFilter::Medium, "90 - 120 BPM (Mid-Tempo)"),
                bpm_btn(BpmFilter::Fast, "120 - 140 BPM (Upbeat)"),
                bpm_btn(BpmFilter::HighEnergy, "> 140 BPM (High Energy)"),
            ]
            .spacing(4)
            .into()
        }
        FilterCategory::All | FilterCategory::Folders => {
            let tree = props.folder_tree;
            if tree.is_empty() {
                column![text("No folders loaded").size(12).style(p.text_muted)].into()
            } else {
                let mut folder_col = column![].spacing(2);
                for node in tree {
                    folder_col = folder_col.push(render_folder_tree_node(node, 0, props));
                }
                folder_col.into()
            }
        }
    };

    // Active filter chip / clear pill
    let mut filter_summary_row: Option<Element<'a, Message>> = None;
    if props.filter_state.is_any_active() {
        let mut desc = String::new();
        if let Some(artist) = &props.filter_state.artist {
            desc.push_str(&format!("Artist: {} ", artist));
        }
        if let Some(album) = &props.filter_state.album {
            desc.push_str(&format!("Album: {} ", album));
        }
        if let Some(genre) = &props.filter_state.genre {
            desc.push_str(&format!("Genre: {} ", genre));
        }
        if let Some(dur) = props.filter_state.duration {
            desc.push_str(&format!("Duration: {} ", dur.label()));
        }
        if let Some(year) = props.filter_state.year {
            desc.push_str(&format!("Year: {} ", year));
        }
        if let Some(bpm) = props.filter_state.bpm {
            desc.push_str(&format!("BPM: {} ", bpm.label()));
        }

        filter_summary_row = Some(
            container(
                row![
                    svg_icon(ICON_FILTER_SVG, 12.0),
                    text(desc.trim()).size(11).style(p.text_primary).width(Length::Fill),
                    button(svg_icon(ICON_CLEAR_SVG, 11.0))
                        .on_press(Message::ClearFilters)
                        .padding([2, 5])
                        .style(btn_invisible_style()),
                ]
                .spacing(4)
                .align_items(Alignment::Center),
            )
            .padding([4, 8])
            .style(badge_container_style(p))
            .into(),
        );
    }

    let mut sidebar_content = column![
        text("NAVIGATION").size(11).style(p.text_muted),
        lib_btn,
        most_played_btn,
        queue_btn,
        manager_btn,
        info_btn,
        text("FILTER BY").size(11).style(p.text_muted),
        cat_chips_1,
        cat_chips_2,
    ]
    .spacing(8);

    if let Some(summary) = filter_summary_row {
        sidebar_content = sidebar_content.push(summary);
    }

    sidebar_content = sidebar_content.push(filter_details);

    // If active category is not Folders or All, also show hierarchical Folders section at bottom
    if props.filter_state.active_category != FilterCategory::Folders && props.filter_state.active_category != FilterCategory::All {
        let tree = props.folder_tree;
        if !tree.is_empty() {
            let mut folder_col = column![
                text("FOLDERS").size(11).style(p.text_muted),
            ]
            .spacing(4);
            for node in tree {
                folder_col = folder_col.push(render_folder_tree_node(node, 0, props));
            }
            sidebar_content = sidebar_content.push(folder_col);
        }
    }

    container(
        scrollable(container(sidebar_content).padding([0, 8]))
            .height(Length::Fill),
    )
        .padding(12)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(panel_container_style(p))
        .into()
}

fn render_folder_tree_node<'a>(
    node: &FolderTreeNode,
    depth: usize,
    props: &ViewProps<'a>,
) -> Element<'a, Message> {
    let p = &props.palette;
    let is_selected = props.selected_folder.map(|f| f == &node.path).unwrap_or(false);
    let has_children = !node.children.is_empty();
    let is_expanded = props.expanded_folders.contains(&node.path);

    let toggle_btn: Element<'a, Message> = if has_children {
        let icon = if is_expanded { ICON_CHEVRON_DOWN_SVG } else { ICON_CHEVRON_RIGHT_SVG };
        button(svg_icon(icon, 10.0))
            .on_press(Message::ToggleFolderExpanded(node.path.clone()))
            .padding([4, 4])
            .style(btn_invisible_style())
            .into()
    } else {
        container(horizontal_space().width(Length::Fixed(14.0))).into()
    };

    let folder_label = button(
        row![
            svg_icon(ICON_FOLDER_SVG, 13.0),
            text(node.name.clone()).size(12).width(Length::Fill),
            text(format!("{}", node.track_count)).size(10).style(p.text_muted),
        ]
        .spacing(4)
        .align_items(Alignment::Center),
    )
    .on_press(Message::SelectFolder(Some(node.path.clone())))
    .padding([4, 6])
    .width(Length::Fill)
    .style(if is_selected { btn_selected_nav_style(p) } else { btn_invisible_style() });

    let play_btn = button(svg_icon(ICON_PLAY_SVG, 9.0))
        .on_press(Message::PlayFolder(node.path.clone()))
        .padding([3, 5])
        .style(btn_play_row_style(p));

    let queue_btn = button(text("+Q").size(10))
        .on_press(Message::QueueFolder(node.path.clone()))
        .padding([2, 4])
        .style(btn_queue_row_style(p));

    let mut row_items = row![].spacing(2).align_items(Alignment::Center);
    if depth > 0 {
        row_items = row_items.push(horizontal_space().width(Length::Fixed((depth * 10) as f32)));
    }
    row_items = row_items.push(toggle_btn).push(folder_label).push(play_btn).push(queue_btn);

    let mut col = column![row_items].spacing(2);
    if has_children && is_expanded {
        for child in &node.children {
            col = col.push(render_folder_tree_node(child, depth + 1, props));
        }
    }
    col.into()
}

fn render_track_list<'a>(props: &ViewProps<'a>) -> Element<'a, Message> {
    let p = &props.palette;

    if props.all_tracks.is_empty() {
        return container(
            column![
                svg_icon(LOGO_SVG, 64.0),
                text("Your Music Library is Empty").size(24).style(p.text_primary),
                text("Add a music directory to index your songs, or generate sample audio tracks to test playback immediately.")
                    .size(14)
                    .style(p.text_secondary),
                row![
                    button(
                        row![
                            svg_icon(ICON_FILE_SVG, 14.0),
                            text("Add Audio Files").size(14).style(p.accent_text),
                        ]
                        .spacing(8)
                        .align_items(Alignment::Center),
                    )
                    .on_press(Message::ImportFiles)
                    .padding([10, 18])
                    .style(btn_primary_style(p)),

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
                    .style(btn_default_style(p)),

                    button(
                        row![
                            svg_icon(ICON_PLAY_SVG, 14.0),
                            text("Sample Audio").size(14),
                        ]
                        .spacing(8)
                        .align_items(Alignment::Center),
                    )
                    .on_press(Message::GenerateSampleAudio)
                    .padding([10, 18])
                    .style(btn_default_style(p)),
                ]
                .spacing(12),
            ]
            .spacing(14)
            .align_items(Alignment::Center),
        )
        .padding(40)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(panel_container_style(p))
        .into();
    }

    // Determine view header
    let is_most_played = props.current_tab == NavTab::MostPlayed;
    let (header_title, header_icon): (String, Element<'a, Message>) = if is_most_played {
        ("Frequently Played Tracks".to_string(), svg_icon(ICON_FIRE_SVG, 22.0))
    } else if let Some(folder) = props.selected_folder {
        let name = folder.file_name().and_then(|n| n.to_str()).unwrap_or("Folder").to_string();
        (format!("Folder: {}", name), svg_icon(ICON_FOLDER_SVG, 18.0))
    } else if props.filter_state.is_any_active() {
        let mut filter_desc = String::new();
        if let Some(artist) = &props.filter_state.artist { filter_desc = format!("Artist: {artist}"); }
        else if let Some(album) = &props.filter_state.album { filter_desc = format!("Album: {album}"); }
        else if let Some(genre) = &props.filter_state.genre { filter_desc = format!("Genre: {genre}"); }
        else if let Some(dur) = props.filter_state.duration { filter_desc = format!("Duration: {}", dur.label()); }
        else if let Some(yr) = props.filter_state.year { filter_desc = format!("Year: {yr}"); }
        else if let Some(bpm) = props.filter_state.bpm { filter_desc = format!("BPM: {}", bpm.label()); }
        (format!("Filter: {}", filter_desc), svg_icon(ICON_FILTER_SVG, 16.0))
    } else if !props.search.is_empty() {
        (format!("Search: \"{}\"", props.search), svg_icon(ICON_LIBRARY_SVG, 18.0))
    } else {
        ("All Tracks".to_string(), svg_icon(ICON_LIBRARY_SVG, 18.0))
    };

    let mut left_title_row = row![
        header_icon,
        text(header_title).size(20).style(p.text_primary),
    ]
    .spacing(8)
    .align_items(Alignment::Center);

    if is_most_played {
        left_title_row = left_title_row
            .push(
                button(
                    row![svg_icon(ICON_PLAY_SVG, 11.0), text("Play All").size(12).style(p.accent_text)]
                        .spacing(4)
                        .align_items(Alignment::Center),
                )
                .on_press(Message::PlayMostPlayed)
                .padding([5, 10])
                .style(btn_primary_style(p)),
            )
            .push(
                button(text("+Q Queue All").size(12))
                    .on_press(Message::QueueMostPlayed)
                    .padding([5, 10])
                    .style(btn_default_style(p)),
            );
    }

    let is_filtered_or_searched = props.selected_folder.is_some()
        || props.filter_state.is_any_active()
        || !props.search.is_empty()
        || !props.selected_track_ids.is_empty()
        || is_most_played;

    let mut right_title_row = row![
        text(format!("{} songs found", props.tracks.len()))
            .size(13)
            .style(p.text_secondary),
    ]
    .spacing(12)
    .align_items(Alignment::Center);

    if is_filtered_or_searched {
        right_title_row = right_title_row.push(
            button(
                row![
                    svg_icon(ICON_CLEAR_SVG, 11.0),
                    text("Clear").size(12),
                ]
                .spacing(4)
                .align_items(Alignment::Center),
            )
            .on_press(Message::ClearAll)
            .padding([5, 10])
            .style(btn_default_style(p)),
        );
    }

    let title_row = row![
        left_title_row,
        horizontal_space(),
        right_title_row,
    ]
    .align_items(Alignment::Center);

    // Multi-Selection Action Bar
    let selection_bar: Option<Element<'a, Message>> = if !props.selected_track_ids.is_empty() {
        Some(
            container(
                row![
                    svg_icon(ICON_CHECK_SVG, 14.0),
                    text(format!("{} songs selected", props.selected_track_ids.len()))
                        .size(13)
                        .style(p.accent_text),
                    horizontal_space(),
                    button(
                        row![svg_icon(ICON_PLAY_SVG, 11.0), text("Play Selected").size(12).style(p.accent_text)]
                            .spacing(4)
                            .align_items(Alignment::Center),
                    )
                    .on_press(Message::PlaySelectedTracks)
                    .padding([5, 10])
                    .style(btn_primary_style(p)),

                    button(text("+Q Add to Queue").size(12))
                        .on_press(Message::QueueSelectedTracks)
                        .padding([5, 10])
                        .style(btn_default_style(p)),

                    button(
                        row![svg_icon(ICON_EDIT_SVG, 12.0), text("Batch Edit Tags").size(12)]
                            .spacing(4)
                            .align_items(Alignment::Center),
                    )
                    .on_press(Message::StartBatchEdit)
                    .padding([5, 10])
                    .style(btn_accent_style(p)),

                    button(
                        row![svg_icon(ICON_TRASH_SVG, 12.0), text("Remove from Library").size(12).style(Color::from_rgb8(248, 113, 113))]
                            .spacing(4)
                            .align_items(Alignment::Center),
                    )
                    .on_press(Message::RemoveSelectedTracksFromLibrary)
                    .padding([5, 10])
                    .style(btn_default_style(p)),

                    button(
                        row![
                            svg_icon(ICON_CLEAR_SVG, 11.0),
                            text("Deselect").size(12),
                        ]
                        .spacing(4)
                        .align_items(Alignment::Center),
                    )
                        .on_press(Message::ClearTrackSelection)
                        .padding([5, 8])
                        .style(btn_default_style(p)),
                ]
                .spacing(8)
                .align_items(Alignment::Center),
            )
            .padding([6, 12])
            .style(selection_bar_style(p))
            .into(),
        )
    } else {
        None
    };

    // Column Headers
    let all_selected = !props.tracks.is_empty() && props.tracks.iter().all(|t| props.selected_track_ids.contains(&t.id));
    let select_all_btn = button(
        container(if all_selected {
            svg_icon(ICON_CHECK_SVG, 11.0)
        } else {
            horizontal_space().into()
        })
        .width(Length::Fixed(16.0))
        .height(Length::Fixed(16.0))
        .style(if all_selected { checkbox_checked_style(p) } else { checkbox_unchecked_style(p) })
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center),
    )
    .on_press(Message::SelectAllVisibleTracks)
    .padding(2)
    .style(btn_invisible_style());

    let col_headers = row![
        select_all_btn,
        text("#").width(Length::Fixed(28.0)).style(p.text_muted),
        text("TITLE").width(Length::FillPortion(4)).style(p.text_muted),
        text("ARTIST").width(Length::FillPortion(3)).style(p.text_muted),
        text("ALBUM").width(Length::FillPortion(3)).style(p.text_muted),
        text("TIME").width(Length::Fixed(60.0)).style(p.text_muted),
        text("ACTIONS").width(Length::Fixed(150.0)).style(p.text_muted),
    ]
    .spacing(8)
    .padding([6, 10]);

    let mut track_rows = column![].spacing(3);
    for (idx, track) in props.tracks.iter().enumerate() {
        let is_playing = props.current_playing_track_id == Some(track.id);
        let is_row_selected = props.selected_track == Some(track.id);
        let is_checked = props.selected_track_ids.contains(&track.id);
        let duration = track.duration.map(format_duration).unwrap_or_else(|| "--:--".into());

        let row_check_btn = button(
            container(if is_checked {
                svg_icon(ICON_CHECK_SVG, 11.0)
            } else {
                horizontal_space().into()
            })
            .width(Length::Fixed(16.0))
            .height(Length::Fixed(16.0))
            .style(if is_checked { checkbox_checked_style(p) } else { checkbox_unchecked_style(p) })
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center),
        )
        .on_press(Message::ToggleTrackSelection(track.id))
        .padding(2)
        .style(btn_invisible_style());

        let icon: Element<'a, Message> = if is_playing {
            container(svg_icon(ICON_PLAY_SVG, 12.0))
                .width(Length::Fixed(24.0))
                .into()
        } else {
            container(text(format!("{:02}", idx + 1)).size(13).style(p.text_muted))
                .width(Length::Fixed(24.0))
                .into()
        };

        let title_style = if is_playing {
            p.accent
        } else if is_row_selected || is_checked {
            p.visualizer_high
        } else {
            p.text_primary
        };

        let mut title_inner = row![
            text(&track.title).size(14).style(title_style),
        ]
        .spacing(6)
        .align_items(Alignment::Center);

        if track.play_count > 0 || is_most_played {
            title_inner = title_inner.push(
                container(
                    row![
                        svg_icon(ICON_FIRE_SVG, 11.0),
                        text(format!("{} plays", track.play_count)).size(10).style(Color::from_rgb8(245, 158, 11)),
                    ]
                    .spacing(3)
                    .align_items(Alignment::Center),
                )
                .padding([2, 5])
                .style(badge_container_style(p)),
            );
        }

        let row_content = row![
            row_check_btn,
            icon,
            container(title_inner).width(Length::FillPortion(4)),
            text(&track.artist).width(Length::FillPortion(3)).size(13).style(p.text_secondary),
            text(&track.album).width(Length::FillPortion(3)).size(13).style(p.text_secondary),
            text(duration).width(Length::Fixed(56.0)).size(13).style(p.text_secondary),
            row![
                button(svg_icon(ICON_PLAY_SVG, 10.0))
                    .on_press(Message::PlayTrack(track.id))
                    .padding([5, 8])
                    .style(btn_play_row_style(p)),
                button(text("+Q").size(11))
                    .on_press(Message::QueueTrack(track.id))
                    .padding([4, 6])
                    .style(btn_queue_row_style(p)),
                button(svg_icon(ICON_EDIT_SVG, 11.0))
                    .on_press(Message::StartEditTrack(track.id))
                    .padding([4, 6])
                    .style(btn_default_style(p)),
                button(svg_icon(ICON_CLEAR_SVG, 10.0))
                    .on_press(Message::RemoveTrackFromLibrary(track.id))
                    .padding([4, 6])
                    .style(btn_default_style(p)),
            ]
            .spacing(4)
            .width(Length::Fixed(146.0)),
        ]
        .spacing(8)
        .align_items(Alignment::Center);

        let row_btn = button(row_content)
            .on_press(Message::PlayTrack(track.id))
            .width(Length::Fill)
            .padding([7, 10])
            .style(if is_playing {
                btn_playing_row_style(p)
            } else if is_row_selected || is_checked {
                btn_selected_row_style(p)
            } else {
                btn_item_row_style(p)
            });

        track_rows = track_rows.push(row_btn);
    }

    let mut content = column![title_row].spacing(8);

    if let Some(bar) = selection_bar {
        content = content.push(bar);
    }

    if props.batch_edit.is_open {
        content = content.push(render_batch_tag_editor(props.batch_edit, props.selected_track_ids.len(), p));
    } else if let Some(editing) = props.editing_track {
        content = content.push(render_tag_editor(editing, p));
    }

    if props.tracks.is_empty() {
        content = content.push(
            container(
                column![
                    text("No tracks match your current filter or search.").size(15).style(p.text_secondary),
                    button(
                        row![
                            svg_icon(ICON_CLEAR_SVG, 13.0),
                            text("Clear View & Filters").size(13).style(p.accent_text),
                        ]
                        .spacing(6)
                        .align_items(Alignment::Center),
                    )
                        .on_press(Message::ClearAll)
                        .padding([8, 16])
                        .style(btn_primary_style(p)),
                ]
                .spacing(12)
                .align_items(Alignment::Center),
            )
            .padding(30)
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center),
        );
    } else {
        content = content.push(col_headers).push(scrollable(track_rows).height(Length::Fill));
    }

    container(content)
        .padding(14)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(panel_container_style(p))
        .into()
}

fn render_batch_tag_editor<'a>(batch: &'a BatchEditState, count: usize, p: &ThemePalette) -> Element<'a, Message> {
    let artist_input = text_input("New Artist for all selected", &batch.artist)
        .on_input(|s| Message::BatchFieldChanged(BatchField::Artist, s))
        .padding(8);
    let album_input = text_input("New Album for all selected", &batch.album)
        .on_input(|s| Message::BatchFieldChanged(BatchField::Album, s))
        .padding(8);
    let genre_input = text_input("New Genre for all selected", &batch.genre)
        .on_input(|s| Message::BatchFieldChanged(BatchField::Genre, s))
        .padding(8);
    let year_input = text_input("Year", &batch.year)
        .on_input(|s| Message::BatchFieldChanged(BatchField::Year, s))
        .padding(8)
        .width(Length::Fixed(80.0));
    let bpm_input = text_input("BPM", &batch.bpm)
        .on_input(|s| Message::BatchFieldChanged(BatchField::Bpm, s))
        .padding(8)
        .width(Length::Fixed(80.0));

    let artist_row = row![
        custom_checkbox(batch.apply_artist, "Update Artist", Message::ToggleBatchFieldApply(BatchField::Artist), p),
        artist_input,
    ]
    .spacing(8)
    .align_items(Alignment::Center);

    let album_row = row![
        custom_checkbox(batch.apply_album, "Update Album", Message::ToggleBatchFieldApply(BatchField::Album), p),
        album_input,
    ]
    .spacing(8)
    .align_items(Alignment::Center);

    let genre_row = row![
        custom_checkbox(batch.apply_genre, "Update Genre", Message::ToggleBatchFieldApply(BatchField::Genre), p),
        genre_input,
    ]
    .spacing(8)
    .align_items(Alignment::Center);

    let year_bpm_row = row![
        custom_checkbox(batch.apply_year, "Update Year", Message::ToggleBatchFieldApply(BatchField::Year), p),
        year_input,
        horizontal_space().width(Length::Fixed(12.0)),
        custom_checkbox(batch.apply_bpm, "Update BPM", Message::ToggleBatchFieldApply(BatchField::Bpm), p),
        bpm_input,
    ]
    .spacing(8)
    .align_items(Alignment::Center);

    let save_btn = button(
        row![
            svg_icon(ICON_EDIT_SVG, 13.0),
            text(format!("Apply Tags to All {} Tracks", count)).size(13).style(p.accent_text),
        ]
        .spacing(6)
        .align_items(Alignment::Center),
    )
    .on_press(Message::SaveBatchTags)
    .padding([8, 16])
    .style(btn_primary_style(p));

    let cancel_btn = button(text("Cancel").size(13))
        .on_press(Message::CancelBatchEdit)
        .padding([8, 14])
        .style(btn_default_style(p));

    container(
        column![
            row![
                svg_icon(ICON_EDIT_SVG, 18.0),
                text(format!("Batch Tag Engine — Editing {} Selected Tracks", count)).size(16).style(p.text_primary),
            ]
            .spacing(8)
            .align_items(Alignment::Center),
            text("Check the box next to any field you want to apply to all selected tracks. Unchecked fields are preserved.")
                .size(12)
                .style(p.text_secondary),
            artist_row,
            album_row,
            genre_row,
            year_bpm_row,
            row![save_btn, cancel_btn].spacing(10),
        ]
        .spacing(12),
    )
    .padding(16)
    .width(Length::Fill)
    .style(card_container_style(p))
    .into()
}

fn render_tag_editor<'a>(editing: &'a EditingTrackState, p: &ThemePalette) -> Element<'a, Message> {
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
        .width(Length::Fixed(70.0));
    let bpm_input = text_input("BPM", &editing.bpm)
        .on_input(|s| Message::EditFieldChanged(EditField::Bpm, s))
        .padding(8)
        .width(Length::Fixed(70.0));

    let save_btn = button(
        row![
            svg_icon(ICON_EDIT_SVG, 13.0),
            text("Save Tags").size(13).style(p.accent_text),
        ]
        .spacing(6)
        .align_items(Alignment::Center),
    )
    .on_press(Message::SaveTrackTags)
    .padding([8, 16])
    .style(btn_primary_style(p));

    let cancel_btn = button(text("Cancel").size(13))
        .on_press(Message::CancelEditTrack)
        .padding([8, 14])
        .style(btn_default_style(p));

    container(
        column![
            row![
                svg_icon(ICON_EDIT_SVG, 16.0),
                text("Edit Track Metadata").size(15).style(p.text_primary),
            ]
            .spacing(8)
            .align_items(Alignment::Center),
            row![
                column![text("Title").size(11).style(p.text_muted), title_input].spacing(4).width(Length::FillPortion(3)),
                column![text("Artist").size(11).style(p.text_muted), artist_input].spacing(4).width(Length::FillPortion(2)),
                column![text("Album").size(11).style(p.text_muted), album_input].spacing(4).width(Length::FillPortion(2)),
                column![text("Genre").size(11).style(p.text_muted), genre_input].spacing(4).width(Length::FillPortion(2)),
                column![text("Year").size(11).style(p.text_muted), year_input].spacing(4).width(Length::Fixed(70.0)),
                column![text("BPM").size(11).style(p.text_muted), bpm_input].spacing(4).width(Length::Fixed(70.0)),
            ]
            .spacing(10),
            row![save_btn, cancel_btn].spacing(8),
        ]
        .spacing(12),
    )
    .padding(14)
    .width(Length::Fill)
    .style(card_container_style(p))
    .into()
}

fn render_queue_list<'a>(props: &ViewProps<'a>) -> Element<'a, Message> {
    let p = &props.palette;

    let header = row![
        row![
            svg_icon(ICON_QUEUE_SVG, 20.0),
            text("Current Playback Queue").size(20).style(p.text_primary),
        ]
        .spacing(8)
        .align_items(Alignment::Center),
        horizontal_space(),
        button(
            row![
                svg_icon(ICON_CLEAR_SVG, 12.0),
                text("Clear Queue").size(12),
            ]
            .spacing(4)
            .align_items(Alignment::Center),
        )
        .on_press(Message::ClearQueue)
        .padding([6, 12])
        .style(btn_default_style(p)),
    ]
    .align_items(Alignment::Center);

    if props.queued_track_ids.is_empty() {
        return container(
            column![
                header,
                column![
                    text("Queue is empty").size(18).style(p.text_primary),
                    text("Add songs from the library by clicking '+Q' to queue them up.")
                        .size(13)
                        .style(p.text_secondary),
                ]
                .spacing(8)
                .align_items(Alignment::Center),
            ]
            .spacing(20),
        )
        .padding(14)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(panel_container_style(p))
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
                text(format!("#{:02}", q_idx + 1)).size(13).style(p.text_muted).width(Length::Fixed(36.0)),
                text(title).size(14).style(p.text_primary).width(Length::FillPortion(4)),
                text(artist).size(13).style(p.text_secondary).width(Length::FillPortion(3)),
                text(dur).size(13).style(p.text_secondary).width(Length::Fixed(60.0)),
                button(
                    row![
                        svg_icon(ICON_PLAY_SVG, 10.0),
                        text("Play").size(11).style(p.accent_text),
                    ]
                    .spacing(4)
                    .align_items(Alignment::Center),
                )
                .on_press(Message::PlayTrack(track_id))
                .padding([4, 8])
                .style(btn_primary_style(p)),
                button(svg_icon(ICON_CLEAR_SVG, 11.0))
                    .on_press(Message::RemoveFromQueue(q_idx))
                    .padding([4, 8])
                    .style(btn_default_style(p)),
            ]
            .spacing(8)
            .align_items(Alignment::Center),
        )
        .padding([8, 12])
        .width(Length::Fill)
        .style(card_container_style(p));

        rows = rows.push(row_item);
    }

    container(
        column![header, scrollable(rows).height(Length::Fill)].spacing(12),
    )
    .padding(14)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(panel_container_style(p))
    .into()
}

fn render_info_view<'a>(props: &ViewProps<'a>) -> Element<'a, Message> {
    let p = &props.palette;

    // Header Title
    let title_row = row![
        svg_icon(ICON_INFO_SVG, 22.0),
        column![
            text("Audio Engine & System Control Center").size(22).style(p.text_primary),
            text("Audio hardware diagnostics, latency buffer tuning, real-time DSP sound coloration, loudness targets, themes & components")
                .size(12)
                .style(p.text_secondary),
        ].spacing(2),
        horizontal_space(),
        container(
            row![
                text("Active Theme:").size(12).style(p.text_secondary),
                text(props.theme.label()).size(12).style(p.accent),
            ].spacing(6)
        )
        .padding([6, 12])
        .style(badge_container_style(p)),
    ]
    .spacing(8)
    .align_items(Alignment::Center);

    // Section 1: Themes & Visual Appearance
    let mut theme_grid = column![].spacing(10);
    for chunk in PlayerTheme::all().chunks(3) {
        let mut row_cards = row![].spacing(10);
        for &t in chunk {
            let t_pal = t.palette();
            let is_selected = props.theme == t;

            let swatch_row = row![
                container(horizontal_space()).width(Length::Fixed(16.0)).height(Length::Fixed(16.0)).style(chip_color_style(t_pal.bg_root)),
                container(horizontal_space()).width(Length::Fixed(16.0)).height(Length::Fixed(16.0)).style(chip_color_style(t_pal.bg_card)),
                container(horizontal_space()).width(Length::Fixed(16.0)).height(Length::Fixed(16.0)).style(chip_color_style(t_pal.accent)),
                container(horizontal_space()).width(Length::Fixed(16.0)).height(Length::Fixed(16.0)).style(chip_color_style(t_pal.visualizer_high)),
            ].spacing(6);

            let status_indicator = if is_selected {
                container(text("ACTIVE").size(10).style(t_pal.badge_text))
                    .padding([2, 6])
                    .style(badge_container_style(p))
            } else {
                container(text("Switch").size(10).style(p.text_secondary))
                    .padding([2, 6])
                    .style(badge_container_style(p))
            };

            let card_body = column![
                row![
                    text(t.label()).size(13).style(if is_selected { p.accent } else { p.text_primary }),
                    horizontal_space(),
                    status_indicator,
                ].align_items(Alignment::Center),
                text(t_pal.description).size(11).style(p.text_secondary),
                swatch_row,
            ].spacing(6);

            let theme_btn = button(card_body)
                .on_press(Message::SetTheme(t))
                .padding(10)
                .width(Length::FillPortion(1))
                .style(if is_selected { btn_selected_row_style(p) } else { btn_item_row_style(p) });

            row_cards = row_cards.push(theme_btn);
        }
        theme_grid = theme_grid.push(row_cards);
    }

    let themes_section = column![
        row![
            svg_icon(ICON_PALETTE_SVG, 16.0),
            text("Player Themes & Visual Appearance").size(16).style(p.accent),
            text("(6 High-Contrast & Cyberpunk Palettes)").size(12).style(p.text_muted),
        ].spacing(8).align_items(Alignment::Center),
        theme_grid,
    ].spacing(10);

    // Section 2: Audio Hardware Output & Latency Tuning
    let dev = props.audio_device_info;
    let hw_specs = row![
        column![
            text(format!("• Output Soundcard: {}", dev.device_name)).size(13).style(p.text_primary),
            text(format!("• Audio Subsystem API: {}", dev.host_api)).size(13).style(p.text_secondary),
            text(format!("• Native Hardware Rate: {} Hz", dev.sample_rate)).size(13).style(p.text_secondary),
        ].spacing(4).width(Length::FillPortion(1)),
        column![
            text(format!("• Channel Configuration: {} (Stereo)", dev.channels)).size(13).style(p.text_primary),
            text(format!("• Sample Representation: 32-bit Float ({})", dev.sample_format)).size(13).style(p.text_secondary),
            text(format!("• Host Audio Buffer: {}", dev.buffer_size_desc)).size(13).style(p.text_secondary),
        ].spacing(4).width(Length::FillPortion(1)),
    ].spacing(16);

    let mut latency_chips = row![].spacing(8).align_items(Alignment::Center);
    latency_chips = latency_chips.push(text("Audio Buffer Latency:").size(12).style(p.text_secondary));
    for &lat in LatencyProfile::all() {
        let is_sel = props.latency_profile == lat;
        latency_chips = latency_chips.push(
            button(text(lat.short_name()).size(11))
                .on_press(Message::SetLatencyProfile(lat))
                .padding([4, 10])
                .style(btn_chip_style(is_sel, p)),
        );
    }
    latency_chips = latency_chips.push(horizontal_space());
    latency_chips = latency_chips.push(
        button(
            row![svg_icon(ICON_REFRESH_SVG, 12.0), text("Reset Audio Stream").size(11)]
                .spacing(4)
                .align_items(Alignment::Center),
        )
        .on_press(Message::ResetAudioDevice)
        .padding([5, 10])
        .style(btn_default_style(p)),
    );

    let hw_card = container(
        column![
            hw_specs,
            latency_chips,
        ].spacing(12)
    )
    .padding(14)
    .width(Length::Fill)
    .style(card_container_style(p));

    let audio_hw_section = column![
        row![
            svg_icon(ICON_SPEAKER_SVG, 16.0),
            text("Audio Hardware Output & Latency Buffer Tuning").size(16).style(p.accent),
        ].spacing(8).align_items(Alignment::Center),
        hw_card,
    ].spacing(8);

    // Section 3: Real-Time DSP Tone Profiles
    let mut eq_cards = row![].spacing(10);
    for &sp in SoundProfile::all() {
        let is_active = props.sound_profile == sp;
        let card = button(
            column![
                row![
                    text(sp.short_name()).size(13).style(if is_active { p.accent } else { p.text_primary }),
                    horizontal_space(),
                    if is_active {
                        container(text("ACTIVE").size(9).style(p.badge_text)).padding([1, 4]).style(badge_container_style(p))
                    } else {
                        container(horizontal_space()).padding([1, 4])
                    }
                ].align_items(Alignment::Center),
                text(sp.description()).size(11).style(p.text_secondary),
            ].spacing(4)
        )
        .on_press(Message::SetSoundProfile(sp))
        .padding(10)
        .width(Length::FillPortion(1))
        .style(if is_active { btn_selected_row_style(p) } else { btn_item_row_style(p) });

        eq_cards = eq_cards.push(card);
    }

    let dsp_section = column![
        row![
            svg_icon(ICON_EQUALIZER_SVG, 16.0),
            text("Real-Time DSP Tone Profiles (Zero-Latency In-Place Coloration)").size(16).style(p.accent),
        ].spacing(8).align_items(Alignment::Center),
        eq_cards,
    ].spacing(8);

    // Section 4: Audio Diagnostics & Calibration Tools
    let diag_actions = row![
        button(
            row![svg_icon(ICON_SPEAKER_SVG, 14.0), text("Play 440 Hz Reference Chime").size(12)]
                .spacing(6)
                .align_items(Alignment::Center),
        )
        .on_press(Message::PlayAudioTestTone)
        .padding([8, 14])
        .style(btn_default_style(p)),

        button(
            row![svg_icon(ICON_SPEAKER_SVG, 14.0), text("Stereo Channel Test (L / R)").size(12)]
                .spacing(6)
                .align_items(Alignment::Center),
        )
        .on_press(Message::PlayChannelTest)
        .padding([8, 14])
        .style(btn_default_style(p)),

        button(
            row![svg_icon(ICON_FIRE_SVG, 14.0), text("Analyze Current Track Loudness (EBU R128)").size(12)]
                .spacing(6)
                .align_items(Alignment::Center),
        )
        .on_press(Message::AnalyzeReplayGain(None))
        .padding([8, 14])
        .style(btn_accent_style(p)),
    ].spacing(10);

    let diag_section = column![
        row![
            svg_icon(ICON_CHIP_SVG, 16.0),
            text("Audio Diagnostics & Hardware Calibration").size(16).style(p.accent),
            text("Verify speaker wiring, channel phase, and reference levels").size(12).style(p.text_muted),
        ].spacing(8).align_items(Alignment::Center),
        diag_actions,
    ].spacing(8);

    // Section 5: Target Loudness Normalization Standards
    let mut loudness_cards = row![].spacing(10);
    for &tl in TargetLoudness::all() {
        let is_sel = props.target_loudness == tl;
        let card = button(
            column![
                row![
                    text(tl.short_name()).size(13).style(if is_sel { p.accent } else { p.text_primary }),
                    horizontal_space(),
                    if is_sel {
                        container(text("ACTIVE").size(9).style(p.badge_text)).padding([1, 4]).style(badge_container_style(p))
                    } else {
                        container(horizontal_space()).padding([1, 4])
                    }
                ].align_items(Alignment::Center),
                text(tl.description()).size(11).style(p.text_secondary),
            ].spacing(4)
        )
        .on_press(Message::SetTargetLoudness(tl))
        .padding(10)
        .width(Length::FillPortion(1))
        .style(if is_sel { btn_selected_row_style(p) } else { btn_item_row_style(p) });

        loudness_cards = loudness_cards.push(card);
    }

    let loudness_section = column![
        row![
            svg_icon(ICON_FIRE_SVG, 16.0),
            text("Target Loudness Reference (ReplayGain)").size(16).style(p.accent),
        ].spacing(8).align_items(Alignment::Center),
        loudness_cards,
    ].spacing(8);

    // Section 6: Comprehensive Track Codec & Metadata Inspector
    let track_inspector_content = if let Some(track) = props.inspecting_track {
        let ext = track.path.extension().and_then(|s| s.to_str()).unwrap_or("unknown").to_uppercase();
        let size_str = if let Ok(meta) = std::fs::metadata(&track.path) {
            let bytes = meta.len();
            if bytes >= 1024 * 1024 {
                format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
            } else {
                format!("{:.0} KB", bytes as f64 / 1024.0)
            }
        } else {
            "--".to_string()
        };
        let dur_str = track.duration.map(format_duration).unwrap_or_else(|| "--:--".to_string());
        let rg_str = if let Some(gain) = props.current_track_gain {
            format!("{:.1} LUFS (Gain: {:+.1} dB, True Peak: {:.2})", gain.integrated_lufs, gain.gain_db, gain.true_peak)
        } else {
            "Not analyzed (click 'Analyze Current Track Loudness')".to_string()
        };

        column![
            row![
                text(format!("Title: {}", track.title)).size(14).style(p.text_primary),
                horizontal_space(),
                container(text(format!("FORMAT: {}", ext)).size(11).style(p.badge_text))
                    .padding([3, 8])
                    .style(badge_container_style(p)),
            ].align_items(Alignment::Center),
            text(format!("Artist: {} • Album: {} • Genre: {}", track.artist, track.album, track.genre)).size(12).style(p.text_secondary),
            text(format!("Path: {}", track.path.display())).size(11).style(p.text_muted),
            row![
                text(format!("Duration: {}", dur_str)).size(12).style(p.text_secondary),
                text("•").size(12).style(p.text_muted),
                text(format!("File Size: {}", size_str)).size(12).style(p.text_secondary),
                text("•").size(12).style(p.text_muted),
                text(format!("Play Count: {} plays", track.play_count)).size(12).style(p.text_secondary),
                text("•").size(12).style(p.text_muted),
                text(format!("Loudness: {}", rg_str)).size(12).style(p.text_secondary),
            ].spacing(6).align_items(Alignment::Center),
        ].spacing(6)
    } else {
        column![
            text("No song currently playing or selected.").size(13).style(p.text_secondary),
            text("Click or play any music track from your library to inspect codec properties, file bitrates, and ReplayGain dynamics.")
                .size(12)
                .style(p.text_muted),
        ].spacing(4)
    };

    let track_inspector_section = column![
        row![
            svg_icon(ICON_FILE_SVG, 16.0),
            text("Now Playing / Track Technical Inspector").size(16).style(p.accent),
        ].spacing(8).align_items(Alignment::Center),
        container(track_inspector_content)
            .padding(14)
            .width(Length::Fill)
            .style(card_container_style(p)),
    ].spacing(8);

    // Section 7: Supported Media Codecs
    let format_specs = column![
        text("Supported Multi-Format Audio & Media Codecs").size(16).style(p.accent),
        text("• Video Song Containers: .mp4, .mkv, .webm, .avi, .mov, .wmv, .flv, .3gp (Plays audio tracks)").size(13).style(p.text_secondary),
        text("• WebM / Matroska: .webm, .mkv (Opus, Vorbis, AAC, PCM audio)").size(13).style(p.text_secondary),
        text("• MPEG Audio: .mp3, .mp2, .mp1 (Layer I, II, III)").size(13).style(p.text_secondary),
        text("• Free Lossless Audio Codec: .flac (Native 16/24-bit lossless)").size(13).style(p.text_secondary),
        text("• Waveform Audio: .wav, .wave (Linear PCM, IEEE float)").size(13).style(p.text_secondary),
        text("• Ogg Bitstream: .ogg, .oga (Vorbis, Opus, FLAC)").size(13).style(p.text_secondary),
        text("• MP4 / Advanced Audio: .m4a, .m4b, .mp4, .aac (AAC, ALAC)").size(13).style(p.text_secondary),
        text("• Apple & Interchange: .aiff, .aif, .caf (AIFF, Core Audio Format)").size(13).style(p.text_secondary),
    ].spacing(6);

    // Section 8: System & D-Bus MPRIS Diagnostics
    let system_diagnostics = column![
        row![
            svg_icon(ICON_INFO_SVG, 16.0),
            text("System Services & Inter-Process Architecture").size(16).style(p.accent),
        ].spacing(8).align_items(Alignment::Center),
        container(
            column![
                row![
                    text("• Linux MPRIS2 D-Bus Service:").size(13).style(p.text_primary),
                    text("org.mpris.MediaPlayer2.kanono_player (Active & Listening)").size(13).style(p.accent),
                ].spacing(6),
                text("• Compositor Protocol: Linux Wayland (XDG-Shell) / X11 with strict UTF-8 string validation (crash-proof null-byte sanitation)").size(13).style(p.text_secondary),
                text("• Audio Concurrency: Lock-free CPAL output ring with zero allocation on audio thread").size(13).style(p.text_secondary),
                text("• Gapless Lookahead Worker: Asynchronous background decoder with atomic generation tokens").size(13).style(p.text_secondary),
            ].spacing(6)
        )
        .padding(14)
        .width(Length::Fill)
        .style(card_container_style(p)),
    ].spacing(8);

    // Section 9: Native Dynamic Components
    let mut comp_rows = column![].spacing(6);
    if props.components.is_empty() {
        comp_rows = comp_rows.push(
            text("No external native components loaded. Place compiled .so dynamic libraries in the components/ folder.")
                .size(13)
                .style(p.text_secondary),
        );
    } else {
        for comp in props.components {
            let row_card = container(
                row![
                    column![
                        text(comp.metadata.name).size(14).style(p.text_primary),
                        text(format!("ID: {} • Version: {}", comp.metadata.id, comp.metadata.version))
                            .size(12)
                            .style(p.text_secondary),
                    ].spacing(2).width(Length::Fill),
                    container(text("ACTIVE").size(11).style(p.badge_text))
                        .padding([4, 8])
                        .style(badge_container_style(p)),
                ]
                .align_items(Alignment::Center),
            )
            .padding([8, 12])
            .width(Length::Fill)
            .style(card_container_style(p));

            comp_rows = comp_rows.push(row_card);
        }
    }

    let components_section = column![
        row![
            svg_icon(ICON_CHIP_SVG, 16.0),
            text(format!("Native Dynamic Components ({})", props.components.len())).size(16).style(p.accent),
            horizontal_space(),
            button(
                row![svg_icon(ICON_REFRESH_SVG, 12.0), text("Reload Components").size(11)]
                    .spacing(4)
                    .align_items(Alignment::Center),
            )
            .on_press(Message::ReloadComponents)
            .padding([4, 10])
            .style(btn_default_style(p)),
        ].spacing(8).align_items(Alignment::Center),
        comp_rows,
    ].spacing(8);

    let scrollable_content = scrollable(
        column![
            title_row,
            themes_section,
            audio_hw_section,
            dsp_section,
            diag_section,
            loudness_section,
            track_inspector_section,
            format_specs,
            system_diagnostics,
            components_section,
        ]
        .spacing(22),
    )
    .height(Length::Fill);

    container(scrollable_content)
        .padding(24)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(panel_container_style(p))
        .into()
}

fn format_total_duration(d: Duration) -> String {
    let total_secs = d.as_secs();
    let hours = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;
    if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else {
        format!("{}m", mins)
    }
}

fn format_relative_time(secs: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let diff = now.saturating_sub(secs);
    if diff < 60 {
        "Just now".to_string()
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else {
        format!("{}d ago", diff / 86400)
    }
}

fn render_library_manager<'a>(props: &ViewProps<'a>) -> Element<'a, Message> {
    let p = &props.palette;

    // 1. Header with title and quick actions
    let mut header_actions = row![
        button(
            row![svg_icon(ICON_FOLDER_SVG, 14.0), text("Add Folder").size(13).style(p.accent_text)]
                .spacing(6)
                .align_items(Alignment::Center),
        )
        .on_press(Message::ImportFolder)
        .padding([7, 14])
        .style(btn_primary_style(p)),

        button(
            row![svg_icon(ICON_FILE_SVG, 14.0), text("Add Files").size(13)]
                .spacing(6)
                .align_items(Alignment::Center),
        )
        .on_press(Message::ImportFiles)
        .padding([7, 14])
        .style(btn_default_style(p)),

        button(
            row![svg_icon(ICON_REFRESH_SVG, 13.0), text("Rescan All Folders").size(13)]
                .spacing(6)
                .align_items(Alignment::Center),
        )
        .on_press(Message::RescanAllFolders)
        .padding([7, 14])
        .style(btn_default_style(p)),

        button(
            row![svg_icon(ICON_BROOM_SVG, 13.0), text("Clean Missing Songs").size(13)]
                .spacing(6)
                .align_items(Alignment::Center),
        )
        .on_press(Message::PruneMissingTracks)
        .padding([7, 14])
        .style(btn_default_style(p)),
    ]
    .spacing(8)
    .align_items(Alignment::Center);

    if props.is_scanning {
        header_actions = header_actions.push(
            container(text("Scanning in progress...").size(12).style(p.accent))
                .padding([6, 10])
                .style(badge_container_style(p)),
        );
    }

    let header = row![
        row![
            svg_icon(ICON_MANAGER_SVG, 22.0),
            column![
                text("Music Library Manager").size(20).style(p.text_primary),
                text("Manage monitored folders, scan music sources, and maintain your library catalog")
                    .size(12)
                    .style(p.text_secondary),
            ]
            .spacing(2),
        ]
        .spacing(8)
        .align_items(Alignment::Center),
        horizontal_space(),
        header_actions,
    ]
    .align_items(Alignment::Center);

    // 2. Statistics Overview Cards
    let stat_card = |label: &'static str, val: String, sub: &'static str| -> Element<'a, Message> {
        container(
            column![
                text(label).size(11).style(p.text_muted),
                text(val).size(20).style(p.accent),
                text(sub).size(11).style(p.text_secondary),
            ]
            .spacing(3),
        )
        .padding([10, 16])
        .width(Length::FillPortion(1))
        .style(card_container_style(p))
        .into()
    };

    let stats_row = row![
        stat_card("TOTAL TRACKS", format!("{}", props.library_stats.total_tracks), "Indexed in database"),
        stat_card("TOTAL DURATION", format_total_duration(props.library_stats.total_duration), "Playback time"),
        stat_card("ARTISTS & ALBUMS", format!("{} / {}", props.library_stats.total_artists, props.library_stats.total_albums), "Artists / Albums"),
        stat_card("MONITORED FOLDERS", format!("{}", props.library_folders.len()), "Configured paths"),
        stat_card("TOTAL PLAYS", format!("{}", props.library_stats.total_plays), "Playback count"),
    ]
    .spacing(10);

    // 3. Monitored Folders Section
    let folders_title = row![
        svg_icon(ICON_FOLDER_SVG, 16.0),
        text("Monitored Music Folders").size(16).style(p.accent),
        horizontal_space(),
        text(format!("{} folder(s) registered", props.library_folders.len())).size(12).style(p.text_secondary),
    ]
    .align_items(Alignment::Center);

    let folders_content: Element<'a, Message> = if props.library_folders.is_empty() {
        container(
            column![
                text("No monitored folders configured yet").size(15).style(p.text_primary),
                text("Add music folders to allow Kanono Player to scan audio files and automatically keep your music library updated.")
                    .size(13)
                    .style(p.text_secondary),
                button(row![svg_icon(ICON_FOLDER_SVG, 14.0), text("Add First Folder").size(13).style(p.accent_text)].spacing(6).align_items(Alignment::Center))
                    .on_press(Message::ImportFolder)
                    .padding([8, 16])
                    .style(btn_primary_style(p)),
            ]
            .spacing(10)
            .align_items(Alignment::Center),
        )
        .padding(24)
        .width(Length::Fill)
        .align_x(iced::alignment::Horizontal::Center)
        .style(card_container_style(p))
        .into()
    } else {
        let mut f_col = column![].spacing(8);
        for f in props.library_folders {
            let status_badge: Element<'a, Message> = if f.exists_on_disk {
                container(text("ACTIVE").size(10).style(p.accent))
                    .padding([3, 7])
                    .style(badge_container_style(p))
                    .into()
            } else {
                container(text("NOT FOUND ON DISK").size(10).style(Color::from_rgb8(248, 113, 113)))
                    .padding([3, 7])
                    .style(badge_container_style(p))
                    .into()
            };

            let scanned_label = if let Some(ts) = f.last_scanned_at {
                format!("Last scanned: {}", format_relative_time(ts))
            } else {
                "Not scanned yet".to_string()
            };

            let folder_card = container(
                row![
                    svg_icon(ICON_FOLDER_SVG, 22.0),
                    column![
                        row![
                            text(f.path.display().to_string()).size(14).style(p.text_primary),
                            status_badge,
                        ]
                        .spacing(8)
                        .align_items(Alignment::Center),
                        row![
                            text(format!("{} song(s) indexed", f.track_count)).size(12).style(p.text_secondary),
                            text("•").size(12).style(p.text_muted),
                            text(scanned_label).size(12).style(p.text_muted),
                        ]
                        .spacing(6)
                        .align_items(Alignment::Center),
                    ]
                    .spacing(4)
                    .width(Length::Fill),
                    button(
                        row![svg_icon(ICON_REFRESH_SVG, 11.0), text("Rescan").size(12)]
                            .spacing(4)
                            .align_items(Alignment::Center),
                    )
                    .on_press(Message::RescanFolder(f.id))
                    .padding([5, 10])
                    .style(btn_default_style(p)),
                    button(
                        row![svg_icon(ICON_TRASH_SVG, 11.0), text("Remove & Delete Songs").size(12).style(Color::from_rgb8(248, 113, 113))]
                            .spacing(4)
                            .align_items(Alignment::Center),
                    )
                    .on_press(Message::RemoveLibraryFolder(f.id, true))
                    .padding([5, 10])
                    .style(btn_default_style(p)),
                    button(
                        row![svg_icon(ICON_CLEAR_SVG, 11.0), text("Remove Folder Only").size(12)]
                            .spacing(4)
                            .align_items(Alignment::Center),
                    )
                    .on_press(Message::RemoveLibraryFolder(f.id, false))
                    .padding([5, 10])
                    .style(btn_default_style(p)),
                ]
                .spacing(12)
                .align_items(Alignment::Center),
            )
            .padding([10, 14])
            .width(Length::Fill)
            .style(card_container_style(p));

            f_col = f_col.push(folder_card);
        }
        f_col.into()
    };

    // 4. Maintenance and database cleanup tools
    let tools_title = row![
        svg_icon(ICON_INFO_SVG, 16.0),
        text("Maintenance & Database Tools").size(16).style(p.accent),
    ]
    .align_items(Alignment::Center);

    let prune_tool = container(
        row![
            column![
                text("Clean Dead / Missing Songs").size(14).style(p.text_primary),
                text("Verify database tracks against local storage. Deletes catalog entries for files that were renamed, deleted, or moved on disk.")
                    .size(12)
                    .style(p.text_secondary),
            ]
            .spacing(2)
            .width(Length::Fill),
            button(
                row![svg_icon(ICON_BROOM_SVG, 13.0), text("Clean Missing Songs").size(12)]
                    .spacing(6)
                    .align_items(Alignment::Center),
            )
            .on_press(Message::PruneMissingTracks)
            .padding([8, 14])
            .style(btn_default_style(p)),
        ]
        .spacing(12)
        .align_items(Alignment::Center),
    )
    .padding([12, 16])
    .width(Length::Fill)
    .style(card_container_style(p));

    let clear_tool_action: Element<'a, Message> = if props.confirm_clear_library {
        row![
            text("Are you sure?").size(12).style(Color::from_rgb8(248, 113, 113)),
            button(text("Yes, Empty Library").size(12).style(Color::WHITE))
                .on_press(Message::ConfirmClearLibrary)
                .padding([6, 12])
                .style(btn_danger_style(p)),
            button(text("Cancel").size(12))
                .on_press(Message::CancelClearLibrary)
                .padding([6, 12])
                .style(btn_default_style(p)),
        ]
        .spacing(6)
        .align_items(Alignment::Center)
        .into()
    } else {
        button(
            row![svg_icon(ICON_TRASH_SVG, 12.0), text("Clear Entire Library").size(12).style(Color::from_rgb8(248, 113, 113))]
                .spacing(6)
                .align_items(Alignment::Center),
        )
        .on_press(Message::PromptClearLibrary)
        .padding([8, 14])
        .style(btn_default_style(p))
        .into()
    };

    let clear_tool = container(
        row![
            column![
                text("Clear Entire Library Database").size(14).style(p.text_primary),
                text("Resets the music catalog by wiping all indexed songs and monitored folders from the local database. Files on disk will NOT be touched.")
                    .size(12)
                    .style(p.text_secondary),
            ]
            .spacing(2)
            .width(Length::Fill),
            clear_tool_action,
        ]
        .spacing(12)
        .align_items(Alignment::Center),
    )
    .padding([12, 16])
    .width(Length::Fill)
    .style(card_container_style(p));

    let content = column![
        header,
        stats_row,
        folders_title,
        folders_content,
        tools_title,
        prune_tool,
        clear_tool,
    ]
    .spacing(16);

    container(scrollable(content).height(Length::Fill))
        .padding(14)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(panel_container_style(p))
        .into()
}

fn render_visualizer<'a>(pcm: &'a [f32], is_playing: bool, p: &ThemePalette) -> Element<'a, Message> {
    const NUM_BARS: usize = 28;
    const MAX_HEIGHT: f32 = 22.0;
    const MIN_HEIGHT: f32 = 3.0;

    let mut bars = row![].spacing(3).align_items(Alignment::End);

    if !is_playing || pcm.is_empty() {
        for _ in 0..NUM_BARS {
            let bar = container(horizontal_space())
                .width(Length::Fixed(4.0))
                .height(Length::Fixed(MIN_HEIGHT))
                .style(visualizer_bar_style(0.0, p));
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
                .style(visualizer_bar_style(norm, p));
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
    let p = &props.palette;

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
        text(track_title).size(14).style(p.text_primary),
        text(track_artist).size(12).style(p.text_secondary),
    ]
    .spacing(2);

    if let Some(gain) = props.current_track_gain {
        let gain_sign = if gain.gain_db >= 0.0 { "+" } else { "" };
        let gain_info = format!("RG: {}{:.1} dB ({:.1} LUFS)", gain_sign, gain.gain_db, gain.integrated_lufs);
        details_col = details_col.push(
            text(gain_info).size(10).style(p.accent),
        );
    }

    let now_playing_info = row![
        container(svg_icon(ICON_DISC_SVG, 22.0))
            .padding([8, 10])
            .style(badge_container_style(p)),
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
    .style(btn_default_style(p));

    let (play_pause_svg, play_pause_label) = if props.is_playing {
        (ICON_PAUSE_SVG, "Pause")
    } else {
        (ICON_PLAY_SVG, "Play")
    };
    let play_btn = button(
        row![
            svg_icon(play_pause_svg, 14.0),
            text(play_pause_label).size(13).style(p.accent_text),
        ]
        .spacing(6)
        .align_items(Alignment::Center),
    )
    .on_press(Message::TogglePlayback)
    .padding([8, 18])
    .style(btn_primary_style(p));

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
    .style(btn_default_style(p));

    let shuffle_style = if props.is_shuffled { btn_active_toggle_style(p) } else { btn_default_style(p) };
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

    let repeat_style = if props.is_repeated { btn_active_toggle_style(p) } else { btn_default_style(p) };
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

    let rg_style = if props.replaygain_enabled { btn_active_toggle_style(p) } else { btn_default_style(p) };
    let rg_btn = button(text("RG").size(11))
        .on_press(Message::ToggleReplayGain)
        .padding([6, 8])
        .style(rg_style);

    let gapless_style = if props.gapless_enabled { btn_active_toggle_style(p) } else { btn_default_style(p) };
    let gapless_btn = button(text("GAPLESS").size(10))
        .on_press(Message::ToggleGapless)
        .padding([6, 8])
        .style(gapless_style);

    let controls_row = row![shuffle_btn, prev_btn, play_btn, next_btn, repeat_btn, rg_btn, gapless_btn]
        .spacing(8)
        .align_items(Alignment::Center);

    let total_secs = props.metadata.duration.map(|d| d.as_secs_f64()).unwrap_or(0.0);
    let (current_slider_val, display_duration) = if let Some(val) = props.seeking_fraction {
        let scrubbed_secs = total_secs * (val as f64);
        (val, Duration::from_secs_f64(scrubbed_secs))
    } else {
        let elapsed_secs = props.elapsed.as_secs_f64();
        let fraction = if total_secs > 0.0 {
            (elapsed_secs / total_secs).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        (fraction, props.elapsed)
    };

    let elapsed_str = format_duration(display_duration);
    let total_str = props.metadata.duration.map(format_duration).unwrap_or_else(|| "--:--".into());

    let seek_slider = slider(0.0..=1.0, current_slider_val, Message::SeekSlide)
        .on_release(Message::SeekRelease)
        .step(0.001_f32)
        .width(Length::Fill);

    let scrub_row = row![
        text(elapsed_str).size(12).style(p.text_secondary),
        seek_slider,
        text(total_str).size(12).style(p.text_secondary),
    ]
    .spacing(10)
    .align_items(Alignment::Center)
    .width(Length::Fill);

    let visualizer = render_visualizer(props.visualizer_pcm, props.is_playing, p);

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
        .style(btn_default_style(p));

    let vol_slider = slider(0.0..=1.0, props.volume, Message::VolumeChanged)
        .step(0.02_f32)
        .width(Length::Fixed(90.0));

    let vol_percent = format!("{:.0}%", props.volume * 100.0);

    let volume_controls = row![
        mute_btn,
        vol_slider,
        text(vol_percent).size(11).style(p.text_secondary).width(Length::Fixed(34.0)),
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
    .style(panel_container_style(p))
    .into()
}

#[derive(Debug, Clone)]
pub struct FolderTreeNode {
    pub name: String,
    pub path: PathBuf,
    pub track_count: usize,
    pub children: Vec<FolderTreeNode>,
}

pub fn build_folder_tree(tracks: &[LibraryTrack]) -> Vec<FolderTreeNode> {
    if tracks.is_empty() {
        return Vec::new();
    }

    let mut direct_counts: HashMap<PathBuf, usize> = HashMap::new();
    for track in tracks {
        if let Some(parent) = track.path.parent() {
            *direct_counts.entry(parent.to_path_buf()).or_insert(0) += 1;
        }
    }

    if direct_counts.is_empty() {
        return Vec::new();
    }

    let mut all_dirs: HashSet<PathBuf> = HashSet::new();
    for dir in direct_counts.keys() {
        let mut curr: Option<&Path> = Some(dir.as_path());
        while let Some(p) = curr {
            all_dirs.insert(p.to_path_buf());
            curr = p.parent();
        }
    }

    let mut total_counts: HashMap<PathBuf, usize> = HashMap::new();
    for track in tracks {
        let mut curr = track.path.parent();
        while let Some(dir) = curr {
            *total_counts.entry(dir.to_path_buf()).or_insert(0) += 1;
            curr = dir.parent();
        }
    }

    let mut children_map: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
    let mut root_dirs: Vec<PathBuf> = Vec::new();

    for dir in &all_dirs {
        match dir.parent() {
            Some(parent) if all_dirs.contains(parent) => {
                children_map.entry(parent.to_path_buf()).or_default().push(dir.clone());
            }
            _ => {
                root_dirs.push(dir.clone());
            }
        }
    }

    for children in children_map.values_mut() {
        children.sort();
    }
    root_dirs.sort();

    // If root directory has 0 direct tracks (e.g. "/" or "C:\"), expand to its children
    let mut effective_roots = Vec::new();
    for root in root_dirs {
        if direct_counts.get(&root).copied().unwrap_or(0) == 0 {
            if let Some(children) = children_map.get(&root) {
                effective_roots.extend(children.clone());
                continue;
            }
        }
        effective_roots.push(root);
    }

    fn build_node(
        current: PathBuf,
        direct_counts: &HashMap<PathBuf, usize>,
        total_counts: &HashMap<PathBuf, usize>,
        children_map: &HashMap<PathBuf, Vec<PathBuf>>,
        is_root: bool,
    ) -> Vec<FolderTreeNode> {
        let mut curr = current;
        if is_root {
            while direct_counts.get(&curr).copied().unwrap_or(0) == 0 {
                if let Some(children) = children_map.get(&curr) {
                    if children.len() == 1 {
                        curr = children[0].clone();
                        continue;
                    }
                }
                break;
            }
        }

        let name = curr
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_else(|| curr.to_str().unwrap_or("Folder"))
            .to_string();

        let track_count = total_counts.get(&curr).copied().unwrap_or(0);

        let mut child_nodes = Vec::new();
        if let Some(children) = children_map.get(&curr) {
            for child in children {
                let nodes = build_node(child.clone(), direct_counts, total_counts, children_map, false);
                child_nodes.extend(nodes);
            }
        }
        child_nodes.sort_by_key(|a| a.name.to_lowercase());

        vec![FolderTreeNode {
            name,
            path: curr,
            track_count,
            children: child_nodes,
        }]
    }

    let mut result = Vec::new();
    for root in effective_roots {
        let nodes = build_node(root, &direct_counts, &total_counts, &children_map, true);
        result.extend(nodes);
    }
    result.sort_by_key(|a| a.name.to_lowercase());
    result
}

fn format_duration(duration: Duration) -> String {
    let total_secs = duration.as_secs();
    let hours = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, mins, secs)
    } else {
        format!("{:02}:{:02}", mins, secs)
    }
}

fn custom_checkbox<'a>(checked: bool, label: &str, on_toggle: Message, p: &ThemePalette) -> Element<'a, Message> {
    let check_icon: Element<'a, Message> = if checked {
        svg_icon(ICON_CHECK_SVG, 11.0)
    } else {
        horizontal_space().into()
    };
    let box_container = container(check_icon)
        .width(Length::Fixed(16.0))
        .height(Length::Fixed(16.0))
        .style(if checked { checkbox_checked_style(p) } else { checkbox_unchecked_style(p) })
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center);

    button(
        row![
            box_container,
            text(label.to_string()).size(12).style(p.text_primary),
        ]
        .spacing(6)
        .align_items(Alignment::Center),
    )
    .on_press(on_toggle)
    .padding([2, 4])
    .style(btn_invisible_style())
    .into()
}

// ==========================================
// THEME-DRIVEN STYLING IMPLEMENTATIONS (ICED 0.12)
// ==========================================

fn chip_color_style(c: Color) -> iced::theme::Container {
    iced::theme::Container::Custom(Box::new(ColorChipStyle(c)))
}

fn selection_bar_style(p: &ThemePalette) -> iced::theme::Container {
    iced::theme::Container::Custom(Box::new(SelectionBarStyle(p.clone())))
}

fn checkbox_checked_style(p: &ThemePalette) -> iced::theme::Container {
    iced::theme::Container::Custom(Box::new(CheckboxCheckedStyle(p.clone())))
}

fn checkbox_unchecked_style(p: &ThemePalette) -> iced::theme::Container {
    iced::theme::Container::Custom(Box::new(CheckboxUncheckedStyle(p.clone())))
}

fn btn_invisible_style() -> iced::theme::Button {
    iced::theme::Button::Custom(Box::new(InvisibleButtonStyle))
}

fn btn_chip_style(is_active: bool, p: &ThemePalette) -> iced::theme::Button {
    if is_active {
        iced::theme::Button::Custom(Box::new(ChipActiveButtonStyle(p.clone())))
    } else {
        iced::theme::Button::Custom(Box::new(ChipInactiveButtonStyle(p.clone())))
    }
}

fn root_background_style(p: &ThemePalette) -> iced::theme::Container {
    iced::theme::Container::Custom(Box::new(RootBgStyle(p.clone())))
}

fn panel_container_style(p: &ThemePalette) -> iced::theme::Container {
    iced::theme::Container::Custom(Box::new(PanelStyle(p.clone())))
}

fn card_container_style(p: &ThemePalette) -> iced::theme::Container {
    iced::theme::Container::Custom(Box::new(CardStyle(p.clone())))
}

fn badge_container_style(p: &ThemePalette) -> iced::theme::Container {
    iced::theme::Container::Custom(Box::new(BadgeStyle(p.clone())))
}

fn btn_primary_style(p: &ThemePalette) -> iced::theme::Button {
    iced::theme::Button::Custom(Box::new(PrimaryButtonStyle(p.clone())))
}

fn btn_default_style(p: &ThemePalette) -> iced::theme::Button {
    iced::theme::Button::Custom(Box::new(DefaultButtonStyle(p.clone())))
}

fn btn_accent_style(p: &ThemePalette) -> iced::theme::Button {
    iced::theme::Button::Custom(Box::new(AccentButtonStyle(p.clone())))
}

fn btn_nav_style(p: &ThemePalette) -> iced::theme::Button {
    iced::theme::Button::Custom(Box::new(NavButtonStyle(p.clone())))
}

fn btn_selected_nav_style(p: &ThemePalette) -> iced::theme::Button {
    iced::theme::Button::Custom(Box::new(SelectedNavButtonStyle(p.clone())))
}

fn btn_item_row_style(p: &ThemePalette) -> iced::theme::Button {
    iced::theme::Button::Custom(Box::new(ItemRowButtonStyle(p.clone())))
}

fn btn_selected_row_style(p: &ThemePalette) -> iced::theme::Button {
    iced::theme::Button::Custom(Box::new(SelectedRowButtonStyle(p.clone())))
}

fn btn_playing_row_style(p: &ThemePalette) -> iced::theme::Button {
    iced::theme::Button::Custom(Box::new(PlayingRowButtonStyle(p.clone())))
}

fn btn_play_row_style(p: &ThemePalette) -> iced::theme::Button {
    iced::theme::Button::Custom(Box::new(PlayRowButtonStyle(p.clone())))
}

fn btn_queue_row_style(p: &ThemePalette) -> iced::theme::Button {
    iced::theme::Button::Custom(Box::new(QueueRowButtonStyle(p.clone())))
}

fn btn_active_toggle_style(p: &ThemePalette) -> iced::theme::Button {
    iced::theme::Button::Custom(Box::new(ActiveToggleButtonStyle(p.clone())))
}

fn btn_danger_style(_p: &ThemePalette) -> iced::theme::Button {
    iced::theme::Button::Custom(Box::new(DangerButtonStyle))
}

fn visualizer_bar_style(ratio: f32, p: &ThemePalette) -> iced::theme::Container {
    iced::theme::Container::Custom(Box::new(VisualizerBarStyle {
        ratio,
        palette: p.clone(),
    }))
}

// Containers
struct ColorChipStyle(Color);
impl iced::widget::container::StyleSheet for ColorChipStyle {
    type Style = iced::Theme;
    fn appearance(&self, _style: &Self::Style) -> iced::widget::container::Appearance {
        iced::widget::container::Appearance {
            background: Some(self.0.into()),
            text_color: None,
            border: iced::Border {
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.25),
                width: 1.0,
                radius: 4.0.into(),
            },
            shadow: iced::Shadow::default(),
        }
    }
}

struct RootBgStyle(ThemePalette);
impl iced::widget::container::StyleSheet for RootBgStyle {
    type Style = iced::Theme;
    fn appearance(&self, _style: &Self::Style) -> iced::widget::container::Appearance {
        iced::widget::container::Appearance {
            background: Some(self.0.bg_root.into()),
            text_color: Some(self.0.text_primary),
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
        }
    }
}

struct PanelStyle(ThemePalette);
impl iced::widget::container::StyleSheet for PanelStyle {
    type Style = iced::Theme;
    fn appearance(&self, _style: &Self::Style) -> iced::widget::container::Appearance {
        iced::widget::container::Appearance {
            background: Some(self.0.bg_panel.into()),
            text_color: Some(self.0.text_primary),
            border: iced::Border {
                color: self.0.border,
                width: 1.0,
                radius: 12.0.into(),
            },
            shadow: iced::Shadow::default(),
        }
    }
}

struct CardStyle(ThemePalette);
impl iced::widget::container::StyleSheet for CardStyle {
    type Style = iced::Theme;
    fn appearance(&self, _style: &Self::Style) -> iced::widget::container::Appearance {
        iced::widget::container::Appearance {
            background: Some(self.0.bg_card.into()),
            text_color: Some(self.0.text_primary),
            border: iced::Border {
                color: self.0.border_subtle,
                width: 1.0,
                radius: 8.0.into(),
            },
            shadow: iced::Shadow::default(),
        }
    }
}

struct BadgeStyle(ThemePalette);
impl iced::widget::container::StyleSheet for BadgeStyle {
    type Style = iced::Theme;
    fn appearance(&self, _style: &Self::Style) -> iced::widget::container::Appearance {
        iced::widget::container::Appearance {
            background: Some(self.0.badge_bg.into()),
            text_color: Some(self.0.badge_text),
            border: iced::Border {
                color: self.0.border_subtle,
                width: 1.0,
                radius: 8.0.into(),
            },
            shadow: iced::Shadow::default(),
        }
    }
}

struct PrimaryButtonStyle(ThemePalette);
impl iced::widget::button::StyleSheet for PrimaryButtonStyle {
    type Style = iced::Theme;
    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(self.0.accent.into()),
            text_color: self.0.accent_text,
            border: iced::Border {
                color: self.0.accent_hover,
                width: 1.0,
                radius: 8.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
    fn hovered(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(self.0.accent_hover.into()),
            text_color: self.0.accent_text,
            border: iced::Border {
                color: self.0.accent,
                width: 1.0,
                radius: 8.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
}

struct DefaultButtonStyle(ThemePalette);
impl iced::widget::button::StyleSheet for DefaultButtonStyle {
    type Style = iced::Theme;
    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(self.0.bg_card.into()),
            text_color: self.0.text_primary,
            border: iced::Border {
                color: self.0.border,
                width: 1.0,
                radius: 8.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
    fn hovered(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(self.0.bg_card_hover.into()),
            text_color: self.0.text_primary,
            border: iced::Border {
                color: self.0.accent,
                width: 1.0,
                radius: 8.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
}

struct AccentButtonStyle(ThemePalette);
impl iced::widget::button::StyleSheet for AccentButtonStyle {
    type Style = iced::Theme;
    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(self.0.badge_bg.into()),
            text_color: self.0.accent,
            border: iced::Border {
                color: self.0.accent,
                width: 1.0,
                radius: 8.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
    fn hovered(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(self.0.accent.into()),
            text_color: self.0.accent_text,
            border: iced::Border {
                color: self.0.accent_hover,
                width: 1.0,
                radius: 8.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
}

struct NavButtonStyle(ThemePalette);
impl iced::widget::button::StyleSheet for NavButtonStyle {
    type Style = iced::Theme;
    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: None,
            text_color: self.0.text_secondary,
            border: iced::Border::default(),
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
    fn hovered(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(self.0.row_hover_bg.into()),
            text_color: self.0.text_primary,
            border: iced::Border {
                color: self.0.border_subtle,
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
}

struct SelectedNavButtonStyle(ThemePalette);
impl iced::widget::button::StyleSheet for SelectedNavButtonStyle {
    type Style = iced::Theme;
    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(self.0.nav_selected_bg.into()),
            text_color: self.0.text_primary,
            border: iced::Border {
                color: self.0.nav_selected_border,
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
}

struct ItemRowButtonStyle(ThemePalette);
impl iced::widget::button::StyleSheet for ItemRowButtonStyle {
    type Style = iced::Theme;
    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(self.0.row_item_bg.into()),
            text_color: self.0.text_primary,
            border: iced::Border {
                color: self.0.border_subtle,
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
    fn hovered(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(self.0.row_hover_bg.into()),
            text_color: self.0.text_primary,
            border: iced::Border {
                color: self.0.border,
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
}

struct SelectedRowButtonStyle(ThemePalette);
impl iced::widget::button::StyleSheet for SelectedRowButtonStyle {
    type Style = iced::Theme;
    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(self.0.row_selected_bg.into()),
            text_color: self.0.text_primary,
            border: iced::Border {
                color: self.0.accent,
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
}

struct PlayingRowButtonStyle(ThemePalette);
impl iced::widget::button::StyleSheet for PlayingRowButtonStyle {
    type Style = iced::Theme;
    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(self.0.row_playing_bg.into()),
            text_color: self.0.text_primary,
            border: iced::Border {
                color: self.0.row_playing_border,
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
    fn hovered(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(self.0.accent_hover.into()),
            text_color: self.0.text_primary,
            border: iced::Border {
                color: self.0.accent,
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
}

struct PlayRowButtonStyle(ThemePalette);
impl iced::widget::button::StyleSheet for PlayRowButtonStyle {
    type Style = iced::Theme;
    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(self.0.accent.into()),
            text_color: self.0.accent_text,
            border: iced::Border {
                color: self.0.accent_hover,
                width: 1.0,
                radius: 4.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
}

struct QueueRowButtonStyle(ThemePalette);
impl iced::widget::button::StyleSheet for QueueRowButtonStyle {
    type Style = iced::Theme;
    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(self.0.bg_card.into()),
            text_color: self.0.text_primary,
            border: iced::Border {
                color: self.0.border,
                width: 1.0,
                radius: 4.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
}

struct ActiveToggleButtonStyle(ThemePalette);
impl iced::widget::button::StyleSheet for ActiveToggleButtonStyle {
    type Style = iced::Theme;
    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(self.0.accent.into()),
            text_color: self.0.accent_text,
            border: iced::Border {
                color: self.0.accent_hover,
                width: 1.0,
                radius: 8.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
}

struct DangerButtonStyle;
impl iced::widget::button::StyleSheet for DangerButtonStyle {
    type Style = iced::Theme;
    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(Color::from_rgb8(185, 28, 28).into()),
            text_color: Color::WHITE,
            border: iced::Border {
                color: Color::from_rgb8(239, 68, 68),
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
    fn hovered(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(Color::from_rgb8(220, 38, 38).into()),
            text_color: Color::WHITE,
            border: iced::Border {
                color: Color::from_rgb8(248, 113, 113),
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
}

struct ChipActiveButtonStyle(ThemePalette);
impl iced::widget::button::StyleSheet for ChipActiveButtonStyle {
    type Style = iced::Theme;
    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(self.0.nav_selected_bg.into()),
            text_color: self.0.accent,
            border: iced::Border {
                color: self.0.accent,
                width: 1.0,
                radius: 12.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
}

struct ChipInactiveButtonStyle(ThemePalette);
impl iced::widget::button::StyleSheet for ChipInactiveButtonStyle {
    type Style = iced::Theme;
    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(self.0.bg_card.into()),
            text_color: self.0.text_secondary,
            border: iced::Border {
                color: self.0.border_subtle,
                width: 1.0,
                radius: 12.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
    fn hovered(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(self.0.bg_card_hover.into()),
            text_color: self.0.text_primary,
            border: iced::Border {
                color: self.0.accent,
                width: 1.0,
                radius: 12.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
}

struct VisualizerBarStyle {
    ratio: f32,
    palette: ThemePalette,
}
impl iced::widget::container::StyleSheet for VisualizerBarStyle {
    type Style = iced::Theme;
    fn appearance(&self, _style: &Self::Style) -> iced::widget::container::Appearance {
        let color = if self.ratio < 0.05 {
            self.palette.border_subtle
        } else if self.ratio < 0.6 {
            self.palette.visualizer_low
        } else if self.ratio < 0.85 {
            self.palette.visualizer_mid
        } else {
            self.palette.visualizer_high
        };

        iced::widget::container::Appearance {
            background: Some(color.into()),
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

struct SelectionBarStyle(ThemePalette);
impl iced::widget::container::StyleSheet for SelectionBarStyle {
    type Style = iced::Theme;
    fn appearance(&self, _style: &Self::Style) -> iced::widget::container::Appearance {
        iced::widget::container::Appearance {
            background: Some(self.0.row_selected_bg.into()),
            text_color: Some(self.0.text_primary),
            border: iced::Border {
                color: self.0.accent,
                width: 1.0,
                radius: 8.0.into(),
            },
            shadow: iced::Shadow::default(),
        }
    }
}

struct CheckboxCheckedStyle(ThemePalette);
impl iced::widget::container::StyleSheet for CheckboxCheckedStyle {
    type Style = iced::Theme;
    fn appearance(&self, _style: &Self::Style) -> iced::widget::container::Appearance {
        iced::widget::container::Appearance {
            background: Some(self.0.accent.into()),
            text_color: Some(self.0.accent_text),
            border: iced::Border {
                color: self.0.accent_hover,
                width: 1.0,
                radius: 3.0.into(),
            },
            shadow: iced::Shadow::default(),
        }
    }
}

struct CheckboxUncheckedStyle(ThemePalette);
impl iced::widget::container::StyleSheet for CheckboxUncheckedStyle {
    type Style = iced::Theme;
    fn appearance(&self, _style: &Self::Style) -> iced::widget::container::Appearance {
        iced::widget::container::Appearance {
            background: Some(self.0.bg_card.into()),
            text_color: Some(self.0.text_primary),
            border: iced::Border {
                color: self.0.border,
                width: 1.0,
                radius: 3.0.into(),
            },
            shadow: iced::Shadow::default(),
        }
    }
}

struct InvisibleButtonStyle;
impl iced::widget::button::StyleSheet for InvisibleButtonStyle {
    type Style = iced::Theme;
    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: None,
            text_color: Color::from_rgb8(203, 213, 225),
            border: iced::Border::default(),
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
    fn hovered(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.08).into()),
            text_color: Color::WHITE,
            border: iced::Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 4.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
}


