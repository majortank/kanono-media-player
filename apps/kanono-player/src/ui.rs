use std::{collections::BTreeSet, path::PathBuf, time::Duration};

use iced::{
    widget::{button, column, container, horizontal_space, row, scrollable, slider, text, text_input},
    Alignment, Color, Element, Length,
};
use kanono_audio_engine::{LibraryTrack, TrackMetadata};

use crate::{Message, NavTab};

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
    pub volume: f32,
    pub is_muted: bool,
    pub metadata: &'a TrackMetadata,
    pub elapsed: Duration,
    pub status_message: Option<&'a str>,
    pub component_count: usize,
}

pub fn player_view<'a>(props: ViewProps<'a>) -> Element<'a, Message> {
    // --- TOP BAR ---
    let logo = row![
        text("🎵").size(22),
        text("KANONO").size(18).style(Color::from_rgb8(16, 185, 129)),
        text("PLAYER").size(18).style(Color::WHITE),
    ]
    .spacing(6)
    .align_items(Alignment::Center);

    let search_bar = text_input("🔍 Search music library by title, artist, album, genre...", props.search)
        .on_input(Message::SearchChanged)
        .padding(9)
        .width(Length::Fill);

    let top_actions = row![
        button(text("📁 Add Folder").size(13))
            .on_press(Message::ImportFolder)
            .padding([8, 14])
            .style(btn_default_style()),
        button(text("✨ Sample Audio").size(13))
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
            text("📚").size(14),
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
            text("📑").size(14),
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
            text("⚙️").size(14),
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
                    text("📂").size(12),
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
                text("🎵").size(48),
                text("Your Music Library is Empty").size(24).style(Color::WHITE),
                text("Add a music directory to index your songs, or generate sample audio tracks to test playback immediately.")
                    .size(14)
                    .style(Color::from_rgb8(148, 163, 184)),
                row![
                    button(text("✨ Generate Sample Audio Tracks").size(14))
                        .on_press(Message::GenerateSampleAudio)
                        .padding([10, 18])
                        .style(btn_primary_style()),
                    button(text("📁 Browse Music Folder").size(14))
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
        text("ACTIONS").width(Length::Fixed(90.0)).style(Color::from_rgb8(100, 116, 139)),
    ]
    .spacing(8)
    .padding([6, 10]);

    let mut track_rows = column![].spacing(3);
    for (idx, track) in props.tracks.iter().enumerate() {
        let is_playing = props.current_playing_track_id == Some(track.id);
        let is_selected = props.selected_track == Some(track.id);
        let duration = track.duration.map(format_duration).unwrap_or_else(|| "--:--".into());

        let icon = if is_playing {
            text("▶").size(14).style(Color::from_rgb8(16, 185, 129))
        } else {
            text(format!("{:02}", idx + 1)).size(13).style(Color::from_rgb8(100, 116, 139))
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
                icon.width(Length::Fixed(28.0)),
                text(&track.title).width(Length::FillPortion(4)).size(14).style(title_style),
                text(&track.artist).width(Length::FillPortion(3)).size(13).style(Color::from_rgb8(148, 163, 184)),
                text(&track.album).width(Length::FillPortion(3)).size(13).style(Color::from_rgb8(148, 163, 184)),
                text(duration).width(Length::Fixed(56.0)).size(13).style(Color::from_rgb8(148, 163, 184)),
                row![
                    button(text("▶").size(11))
                        .on_press(Message::PlayTrack(track.id))
                        .padding([4, 8])
                        .style(btn_play_row_style()),
                    button(text("+Q").size(11))
                        .on_press(Message::QueueTrack(track.id))
                        .padding([4, 6])
                        .style(btn_queue_row_style()),
                ]
                .spacing(4)
                .width(Length::Fixed(84.0)),
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

    container(
        column![
            title_row,
            col_headers,
            scrollable(track_rows).height(Length::Fill),
        ]
        .spacing(8),
    )
    .padding(14)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(panel_container_style())
    .into()
}

fn render_queue_list<'a>(props: &ViewProps<'a>) -> Element<'a, Message> {
    let header = row![
        text("📑 Current Playback Queue").size(20).style(Color::WHITE),
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
                button(text("▶ Play").size(11))
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
    container(
        column![
            text("⚙️ Audio Engine & System Information").size(22).style(Color::WHITE),
            column![
                text("Architecture & Specifications").size(16).style(Color::from_rgb8(52, 211, 153)),
                text("• High-resolution gapless output pipeline via CPAL (PipeWire / ALSA / JACK)").size(13).style(Color::from_rgb8(203, 213, 225)),
                text("• Pure multi-format audio decoding via Symphonia (MP3, FLAC, WAV, PCM)").size(13).style(Color::from_rgb8(203, 213, 225)),
                text("• Sample-accurate position tracking with lock-free atomic playback clocks").size(13).style(Color::from_rgb8(203, 213, 225)),
                text("• Real-time software volume scaling & instantaneous seeking").size(13).style(Color::from_rgb8(203, 213, 225)),
                text("• EBU R128 Loudness normalization & ReplayGain tagging").size(13).style(Color::from_rgb8(203, 213, 225)),
                text(format!("• Active Dynamic Components: {} loaded", props.component_count)).size(13).style(Color::from_rgb8(203, 213, 225)),
            ].spacing(6),
            column![
                text("Shortcuts & Controls").size(16).style(Color::from_rgb8(52, 211, 153)),
                text("• Click any track to immediately start playback").size(13).style(Color::from_rgb8(203, 213, 225)),
                text("• Drag the progress slider to scrub / seek in the track").size(13).style(Color::from_rgb8(203, 213, 225)),
                text("• Toggle 🔀 for Shuffle and 🔁 for Repeat").size(13).style(Color::from_rgb8(203, 213, 225)),
                text("• Adjust the volume slider or click 🔊 to mute/unmute").size(13).style(Color::from_rgb8(203, 213, 225)),
            ].spacing(6),
        ]
        .spacing(18),
    )
    .padding(24)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(panel_container_style())
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

    let now_playing_info = row![
        container(text("💿").size(24))
            .padding([8, 10])
            .style(badge_container_style()),
        column![
            text(track_title).size(15).style(Color::WHITE),
            text(track_artist).size(12).style(Color::from_rgb8(148, 163, 184)),
        ]
        .spacing(2),
    ]
    .spacing(12)
    .align_items(Alignment::Center)
    .width(Length::Fixed(240.0));

    // Center: Playback Buttons + Seek Slider
    let prev_btn = button(text("⏮").size(16))
        .on_press(Message::Previous)
        .padding([6, 12])
        .style(btn_default_style());

    let play_pause_icon = if props.is_playing { "⏸" } else { "▶" };
    let play_btn = button(text(play_pause_icon).size(18))
        .on_press(Message::TogglePlayback)
        .padding([8, 18])
        .style(btn_primary_style());

    let next_btn = button(text("⏭").size(16))
        .on_press(Message::Next)
        .padding([6, 12])
        .style(btn_default_style());

    let shuffle_style = if props.is_shuffled { btn_active_toggle_style() } else { btn_default_style() };
    let shuffle_btn = button(text("🔀").size(14))
        .on_press(Message::ToggleShuffle)
        .padding([6, 10])
        .style(shuffle_style);

    let repeat_style = if props.is_repeated { btn_active_toggle_style() } else { btn_default_style() };
    let repeat_btn = button(text("🔁").size(14))
        .on_press(Message::ToggleRepeat)
        .padding([6, 10])
        .style(repeat_style);

    let controls_row = row![shuffle_btn, prev_btn, play_btn, next_btn, repeat_btn]
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

    let center_controls = column![controls_row, scrub_row]
        .spacing(6)
        .align_items(Alignment::Center)
        .width(Length::Fill);

    // Right: Volume & Output Info
    let mute_icon = if props.is_muted || props.volume == 0.0 { "🔇" } else { "🔊" };
    let mute_btn = button(text(mute_icon).size(14))
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
