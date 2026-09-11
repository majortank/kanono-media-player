use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    time::Duration,
};

use iced::{
    widget::{button, column, container, horizontal_space, pane_grid, row, scrollable, slider, svg, text, text_input},
    Alignment, Color, Element, Length,
};
use kanono_audio_engine::{LibraryTrack, ReplayGainResult, TrackMetadata};

use crate::{
    plugins::RegisteredPlugin, BatchEditState, BatchField, BpmFilter, DurationFilter,
    EditField, EditingTrackState, FilterCategory, FilterState, Message, NavTab,
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
}

#[derive(Debug, Clone, Copy)]
pub enum PlayerPane {
    Sidebar,
    Main,
}

pub fn player_view<'a>(props: ViewProps<'a>, panes: &'a pane_grid::State<PlayerPane>) -> Element<'a, Message> {
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
                svg_icon(ICON_FILE_SVG, 15.0),
                text("Add Files").size(13),
            ]
            .spacing(6)
            .align_items(Alignment::Center),
        )
        .on_press(Message::ImportFiles)
        .padding([8, 14])
        .style(btn_default_style()),

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

    let middle = pane_grid::PaneGrid::new(panes, |_, pane, _| {
        pane_grid::Content::new(match pane {
            PlayerPane::Sidebar => render_sidebar(&props),
            PlayerPane::Main => match props.current_tab {
                NavTab::Library | NavTab::Folders | NavTab::MostPlayed => render_track_list(&props),
                NavTab::Queue => render_queue_list(&props),
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
    .style(root_background_style())
    .into()
}

fn render_sidebar<'a>(props: &ViewProps<'a>) -> Element<'a, Message> {
    let is_lib_active = props.current_tab == NavTab::Library
        && props.selected_folder.is_none()
        && !props.filter_state.is_any_active();
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

    let most_played_count = props.all_tracks.iter().filter(|t| t.play_count > 0).count();
    let is_most_played_active = props.current_tab == NavTab::MostPlayed;
    let most_played_btn = button(
        row![
            svg_icon(ICON_FIRE_SVG, 16.0),
            text("Most Played").size(14).width(Length::Fill),
            text(format!("{}", most_played_count)).size(11).style(Color::from_rgb8(245, 158, 11)),
        ]
        .spacing(8)
        .align_items(Alignment::Center),
    )
    .on_press(Message::SelectTab(NavTab::MostPlayed))
    .width(Length::Fill)
    .padding([8, 12])
    .style(if is_most_played_active { btn_selected_nav_style() } else { btn_nav_style() });

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

    // Category Selector
    let cat_btn = |cat: FilterCategory, label: &'static str| -> Element<'a, Message> {
        let is_active = props.filter_state.active_category == cat;
        button(text(label).size(11))
            .on_press(Message::SelectFilterCategory(cat))
            .padding([4, 8])
            .style(btn_chip_style(is_active))
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
                        text(format!("{}", count)).size(10).style(Color::from_rgb8(148, 163, 184)),
                    ]
                    .spacing(4)
                    .align_items(Alignment::Center),
                )
                .on_press(Message::SetArtistFilter(if is_sel { None } else { Some(artist) }))
                .padding([4, 8])
                .width(Length::Fill)
                .style(if is_sel { btn_selected_nav_style() } else { btn_invisible_style() });
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
                        text(format!("{}", count)).size(10).style(Color::from_rgb8(148, 163, 184)),
                    ]
                    .spacing(4)
                    .align_items(Alignment::Center),
                )
                .on_press(Message::SetAlbumFilter(if is_sel { None } else { Some(album) }))
                .padding([4, 8])
                .width(Length::Fill)
                .style(if is_sel { btn_selected_nav_style() } else { btn_invisible_style() });
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
                        text(format!("{}", count)).size(10).style(Color::from_rgb8(148, 163, 184)),
                    ]
                    .spacing(4)
                    .align_items(Alignment::Center),
                )
                .on_press(Message::SetGenreFilter(if is_sel { None } else { Some(genre) }))
                .padding([4, 8])
                .width(Length::Fill)
                .style(if is_sel { btn_selected_nav_style() } else { btn_invisible_style() });
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
                    .style(if is_sel { btn_selected_nav_style() } else { btn_default_style() })
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
                column![text("No year metadata found").size(12).style(Color::from_rgb8(148, 163, 184))].into()
            } else {
                let mut year_list = column![].spacing(3);
                for (year, count) in years {
                    let is_sel = props.filter_state.year == Some(year);
                    let btn = button(
                        row![
                            text(format!("{}", year)).size(12).width(Length::Fill),
                            text(format!("{}", count)).size(10).style(Color::from_rgb8(148, 163, 184)),
                        ]
                        .spacing(4)
                        .align_items(Alignment::Center),
                    )
                    .on_press(Message::SetYearFilter(if is_sel { None } else { Some(year) }))
                    .padding([4, 8])
                    .width(Length::Fill)
                    .style(if is_sel { btn_selected_nav_style() } else { btn_invisible_style() });
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
                    .style(if is_sel { btn_selected_nav_style() } else { btn_default_style() })
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
            let tree = build_folder_tree(props.all_tracks);
            if tree.is_empty() {
                column![text("No folders loaded").size(12).style(Color::from_rgb8(148, 163, 184))].into()
            } else {
                let mut folder_col = column![].spacing(2);
                for node in &tree {
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
                    text(desc.trim()).size(11).style(Color::WHITE).width(Length::Fill),
                    button(text("✕").size(11))
                        .on_press(Message::ClearFilters)
                        .padding([2, 5])
                        .style(btn_invisible_style()),
                ]
                .spacing(4)
                .align_items(Alignment::Center),
            )
            .padding([4, 8])
            .style(badge_container_style())
            .into(),
        );
    }

    let mut sidebar_content = column![
        text("NAVIGATION").size(11).style(Color::from_rgb8(100, 116, 139)),
        lib_btn,
        most_played_btn,
        queue_btn,
        info_btn,
        text("FILTER BY").size(11).style(Color::from_rgb8(100, 116, 139)),
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
        let tree = build_folder_tree(props.all_tracks);
        if !tree.is_empty() {
            let mut folder_col = column![
                text("FOLDERS").size(11).style(Color::from_rgb8(100, 116, 139)),
            ]
            .spacing(4);
            for node in &tree {
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
        .style(panel_container_style())
        .into()
}

fn render_folder_tree_node<'a>(
    node: &FolderTreeNode,
    depth: usize,
    props: &ViewProps<'a>,
) -> Element<'a, Message> {
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
            text(format!("{}", node.track_count)).size(10).style(Color::from_rgb8(148, 163, 184)),
        ]
        .spacing(4)
        .align_items(Alignment::Center),
    )
    .on_press(Message::SelectFolder(Some(node.path.clone())))
    .padding([4, 6])
    .width(Length::Fill)
    .style(if is_selected { btn_selected_nav_style() } else { btn_invisible_style() });

    let play_btn = button(svg_icon(ICON_PLAY_SVG, 9.0))
        .on_press(Message::PlayFolder(node.path.clone()))
        .padding([3, 5])
        .style(btn_play_row_style());

    let queue_btn = button(text("+Q").size(10))
        .on_press(Message::QueueFolder(node.path.clone()))
        .padding([2, 4])
        .style(btn_queue_row_style());

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
                            svg_icon(ICON_FILE_SVG, 14.0),
                            text("Add Audio Files").size(14).style(Color::WHITE),
                        ]
                        .spacing(8)
                        .align_items(Alignment::Center),
                    )
                    .on_press(Message::ImportFiles)
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
        text(header_title).size(20).style(Color::WHITE),
    ]
    .spacing(8)
    .align_items(Alignment::Center);

    if is_most_played {
        left_title_row = left_title_row
            .push(
                button(
                    row![svg_icon(ICON_PLAY_SVG, 11.0), text("Play All").size(12).style(Color::WHITE)]
                        .spacing(4)
                        .align_items(Alignment::Center),
                )
                .on_press(Message::PlayMostPlayed)
                .padding([5, 10])
                .style(btn_primary_style()),
            )
            .push(
                button(text("+Q Queue All").size(12))
                    .on_press(Message::QueueMostPlayed)
                    .padding([5, 10])
                    .style(btn_default_style()),
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
            .style(Color::from_rgb8(148, 163, 184)),
    ]
    .spacing(12)
    .align_items(Alignment::Center);

    if is_filtered_or_searched {
        right_title_row = right_title_row.push(
            button(
                row![
                    text("✕ Clear").size(12),
                ]
                .spacing(4)
                .align_items(Alignment::Center),
            )
            .on_press(Message::ClearAll)
            .padding([5, 10])
            .style(btn_default_style()),
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
                        .style(Color::WHITE),
                    horizontal_space(),
                    button(
                        row![svg_icon(ICON_PLAY_SVG, 11.0), text("Play Selected").size(12).style(Color::WHITE)]
                            .spacing(4)
                            .align_items(Alignment::Center),
                    )
                    .on_press(Message::PlaySelectedTracks)
                    .padding([5, 10])
                    .style(btn_primary_style()),

                    button(text("+Q Add to Queue").size(12))
                        .on_press(Message::QueueSelectedTracks)
                        .padding([5, 10])
                        .style(btn_default_style()),

                    button(
                        row![svg_icon(ICON_EDIT_SVG, 12.0), text("Batch Edit Tags").size(12)]
                            .spacing(4)
                            .align_items(Alignment::Center),
                    )
                    .on_press(Message::StartBatchEdit)
                    .padding([5, 10])
                    .style(btn_accent_style()),

                    button(text("✕ Deselect").size(12))
                        .on_press(Message::ClearTrackSelection)
                        .padding([5, 8])
                        .style(btn_default_style()),
                ]
                .spacing(8)
                .align_items(Alignment::Center),
            )
            .padding([6, 12])
            .style(selection_bar_style())
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
        .style(if all_selected { checkbox_checked_style() } else { checkbox_unchecked_style() })
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center),
    )
    .on_press(Message::SelectAllVisibleTracks)
    .padding(2)
    .style(btn_invisible_style());

    let col_headers = row![
        select_all_btn,
        text("#").width(Length::Fixed(28.0)).style(Color::from_rgb8(100, 116, 139)),
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
            .style(if is_checked { checkbox_checked_style() } else { checkbox_unchecked_style() })
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
            container(text(format!("{:02}", idx + 1)).size(13).style(Color::from_rgb8(100, 116, 139)))
                .width(Length::Fixed(24.0))
                .into()
        };

        let title_style = if is_playing {
            Color::from_rgb8(52, 211, 153)
        } else if is_row_selected || is_checked {
            Color::from_rgb8(96, 165, 250)
        } else {
            Color::WHITE
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
                .style(badge_container_style()),
            );
        }

        let row_content = row![
            row_check_btn,
            icon,
            container(title_inner).width(Length::FillPortion(4)),
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
        .align_items(Alignment::Center);

        let row_btn = button(row_content)
            .on_press(Message::PlayTrack(track.id))
            .width(Length::Fill)
            .padding([7, 10])
            .style(if is_playing {
                btn_playing_row_style()
            } else if is_row_selected || is_checked {
                btn_selected_row_style()
            } else {
                btn_item_row_style()
            });

        track_rows = track_rows.push(row_btn);
    }

    let mut content = column![title_row].spacing(8);

    if let Some(bar) = selection_bar {
        content = content.push(bar);
    }

    if props.batch_edit.is_open {
        content = content.push(render_batch_tag_editor(props.batch_edit, props.selected_track_ids.len()));
    } else if let Some(editing) = props.editing_track {
        content = content.push(render_tag_editor(editing));
    }

    if props.tracks.is_empty() {
        content = content.push(
            container(
                column![
                    text("No tracks match your current filter or search.").size(15).style(Color::from_rgb8(148, 163, 184)),
                    button(text("Clear View & Filters").size(13))
                        .on_press(Message::ClearAll)
                        .padding([8, 16])
                        .style(btn_primary_style()),
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
        .style(panel_container_style())
        .into()
}

fn render_batch_tag_editor<'a>(batch: &'a BatchEditState, count: usize) -> Element<'a, Message> {
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
        custom_checkbox(batch.apply_artist, "Update Artist", Message::ToggleBatchFieldApply(BatchField::Artist)),
        artist_input,
    ]
    .spacing(8)
    .align_items(Alignment::Center);

    let album_row = row![
        custom_checkbox(batch.apply_album, "Update Album", Message::ToggleBatchFieldApply(BatchField::Album)),
        album_input,
    ]
    .spacing(8)
    .align_items(Alignment::Center);

    let genre_row = row![
        custom_checkbox(batch.apply_genre, "Update Genre", Message::ToggleBatchFieldApply(BatchField::Genre)),
        genre_input,
    ]
    .spacing(8)
    .align_items(Alignment::Center);

    let year_bpm_row = row![
        custom_checkbox(batch.apply_year, "Update Year", Message::ToggleBatchFieldApply(BatchField::Year)),
        year_input,
        horizontal_space().width(Length::Fixed(12.0)),
        custom_checkbox(batch.apply_bpm, "Update BPM", Message::ToggleBatchFieldApply(BatchField::Bpm)),
        bpm_input,
    ]
    .spacing(8)
    .align_items(Alignment::Center);

    let save_btn = button(
        row![
            svg_icon(ICON_EDIT_SVG, 13.0),
            text(format!("Apply Tags to All {} Tracks", count)).size(13).style(Color::WHITE),
        ]
        .spacing(6)
        .align_items(Alignment::Center),
    )
    .on_press(Message::SaveBatchTags)
    .padding([8, 16])
    .style(btn_primary_style());

    let cancel_btn = button(text("Cancel").size(13))
        .on_press(Message::CancelBatchEdit)
        .padding([8, 14])
        .style(btn_default_style());

    container(
        column![
            row![
                svg_icon(ICON_EDIT_SVG, 18.0),
                text(format!("Batch Tag Engine — Editing {} Selected Tracks", count)).size(16).style(Color::WHITE),
            ]
            .spacing(8)
            .align_items(Alignment::Center),
            text("Check the box next to any field you want to apply to all selected tracks. Unchecked fields are preserved.")
                .size(12)
                .style(Color::from_rgb8(148, 163, 184)),
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
    .style(card_container_style())
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
        .width(Length::Fixed(70.0));
    let bpm_input = text_input("BPM", &editing.bpm)
        .on_input(|s| Message::EditFieldChanged(EditField::Bpm, s))
        .padding(8)
        .width(Length::Fixed(70.0));

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
                text("Edit Track Metadata").size(15).style(Color::WHITE),
            ]
            .spacing(8)
            .align_items(Alignment::Center),
            row![
                column![text("Title").size(11).style(Color::from_rgb8(148, 163, 184)), title_input].spacing(4).width(Length::FillPortion(3)),
                column![text("Artist").size(11).style(Color::from_rgb8(148, 163, 184)), artist_input].spacing(4).width(Length::FillPortion(2)),
                column![text("Album").size(11).style(Color::from_rgb8(148, 163, 184)), album_input].spacing(4).width(Length::FillPortion(2)),
                column![text("Genre").size(11).style(Color::from_rgb8(148, 163, 184)), genre_input].spacing(4).width(Length::FillPortion(2)),
                column![text("Year").size(11).style(Color::from_rgb8(148, 163, 184)), year_input].spacing(4).width(Length::Fixed(70.0)),
                column![text("BPM").size(11).style(Color::from_rgb8(148, 163, 184)), bpm_input].spacing(4).width(Length::Fixed(70.0)),
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
    for dir in &all_dirs {
        let count = tracks.iter().filter(|t| t.path.starts_with(dir)).count();
        total_counts.insert(dir.clone(), count);
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

fn custom_checkbox<'a>(checked: bool, label: &str, on_toggle: Message) -> Element<'a, Message> {
    let check_icon: Element<'a, Message> = if checked {
        svg_icon(ICON_CHECK_SVG, 11.0)
    } else {
        horizontal_space().into()
    };
    let box_container = container(check_icon)
        .width(Length::Fixed(16.0))
        .height(Length::Fixed(16.0))
        .style(if checked { checkbox_checked_style() } else { checkbox_unchecked_style() })
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center);

    button(
        row![
            box_container,
            text(label.to_string()).size(12).style(Color::from_rgb8(226, 232, 240)),
        ]
        .spacing(6)
        .align_items(Alignment::Center),
    )
    .on_press(on_toggle)
    .padding([2, 4])
    .style(btn_invisible_style())
    .into()
}

fn selection_bar_style() -> iced::theme::Container {
    iced::theme::Container::Custom(Box::new(SelectionBarStyle))
}

fn checkbox_checked_style() -> iced::theme::Container {
    iced::theme::Container::Custom(Box::new(CheckboxCheckedStyle))
}

fn checkbox_unchecked_style() -> iced::theme::Container {
    iced::theme::Container::Custom(Box::new(CheckboxUncheckedStyle))
}

fn btn_invisible_style() -> iced::theme::Button {
    iced::theme::Button::Custom(Box::new(InvisibleButtonStyle))
}

fn btn_chip_style(is_active: bool) -> iced::theme::Button {
    if is_active {
        iced::theme::Button::Custom(Box::new(ChipActiveButtonStyle))
    } else {
        iced::theme::Button::Custom(Box::new(ChipInactiveButtonStyle))
    }
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

struct SelectionBarStyle;
impl iced::widget::container::StyleSheet for SelectionBarStyle {
    type Style = iced::Theme;
    fn appearance(&self, _style: &Self::Style) -> iced::widget::container::Appearance {
        iced::widget::container::Appearance {
            background: Some(Color::from_rgb8(20, 30, 48).into()),
            text_color: Some(Color::WHITE),
            border: iced::Border {
                color: Color::from_rgb8(59, 130, 246),
                width: 1.0,
                radius: 8.0.into(),
            },
            shadow: iced::Shadow::default(),
        }
    }
}

struct CheckboxCheckedStyle;
impl iced::widget::container::StyleSheet for CheckboxCheckedStyle {
    type Style = iced::Theme;
    fn appearance(&self, _style: &Self::Style) -> iced::widget::container::Appearance {
        iced::widget::container::Appearance {
            background: Some(Color::from_rgb8(16, 185, 129).into()),
            text_color: Some(Color::WHITE),
            border: iced::Border {
                color: Color::from_rgb8(52, 211, 153),
                width: 1.0,
                radius: 3.0.into(),
            },
            shadow: iced::Shadow::default(),
        }
    }
}

struct CheckboxUncheckedStyle;
impl iced::widget::container::StyleSheet for CheckboxUncheckedStyle {
    type Style = iced::Theme;
    fn appearance(&self, _style: &Self::Style) -> iced::widget::container::Appearance {
        iced::widget::container::Appearance {
            background: Some(Color::from_rgb8(30, 41, 59).into()),
            text_color: Some(Color::WHITE),
            border: iced::Border {
                color: Color::from_rgb8(71, 85, 105),
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
            background: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.07).into()),
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

struct ChipActiveButtonStyle;
impl iced::widget::button::StyleSheet for ChipActiveButtonStyle {
    type Style = iced::Theme;
    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(Color::from_rgb8(16, 185, 129).into()),
            text_color: Color::WHITE,
            border: iced::Border {
                color: Color::from_rgb8(52, 211, 153),
                width: 1.0,
                radius: 12.0.into(),
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
                radius: 12.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
}

struct ChipInactiveButtonStyle;
impl iced::widget::button::StyleSheet for ChipInactiveButtonStyle {
    type Style = iced::Theme;
    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(Color::from_rgb8(24, 30, 44).into()),
            text_color: Color::from_rgb8(148, 163, 184),
            border: iced::Border {
                color: Color::from_rgb8(42, 52, 72),
                width: 1.0,
                radius: 12.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
    fn hovered(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(Color::from_rgb8(33, 41, 58).into()),
            text_color: Color::WHITE,
            border: iced::Border {
                color: Color::from_rgb8(71, 85, 105),
                width: 1.0,
                radius: 12.0.into(),
            },
            shadow_offset: iced::Vector::default(),
            shadow: iced::Shadow::default(),
        }
    }
}


