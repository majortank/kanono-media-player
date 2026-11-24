use std::thread;

use image::GenericImageView;
use ksni::{blocking::TrayMethods, menu::StandardItem};

use crate::mpris::MprisService;

#[derive(Debug, Default)]
struct KanonoTray;

impl ksni::Tray for KanonoTray {
    fn id(&self) -> String {
        "kanono-media-player".into()
    }

    fn title(&self) -> String {
        "Kanono Media Player".into()
    }

    fn icon_name(&self) -> String {
        "kanono-media-player".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        let Ok(image) = image::load_from_memory(include_bytes!("../../../assets/icons/kanono-player.png")) else {
            return Vec::new();
        };
        let (width, height) = image.dimensions();
        let mut data = image.into_rgba8().into_vec();
        for pixel in data.chunks_exact_mut(4) {
            pixel.rotate_right(1);
        }
        vec![ksni::Icon {
            width: width as i32,
            height: height as i32,
            data,
        }]
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        MprisService::raise_existing();
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        vec![
            StandardItem {
                label: "Show Kanono Media Player".into(),
                activate: Box::new(|_| MprisService::raise_existing()),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Play / Pause".into(),
                activate: Box::new(|_| MprisService::play_pause_existing()),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Previous".into(),
                activate: Box::new(|_| MprisService::previous_existing()),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Next".into(),
                activate: Box::new(|_| MprisService::next_existing()),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Seek back 10 seconds".into(),
                activate: Box::new(|_| MprisService::seek_existing(-10_000_000)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Seek forward 10 seconds".into(),
                activate: Box::new(|_| MprisService::seek_existing(10_000_000)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|_| std::process::exit(0)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

pub fn spawn() {
    thread::Builder::new()
        .name("kanono-tray".into())
        .spawn(|| {
            let tray = KanonoTray;
            match tray.spawn() {
                Ok(_handle) => loop {
                    thread::park();
                },
                Err(error) => eprintln!("tray icon unavailable: {error}"),
            }
        })
        .expect("failed to start tray thread");
}
