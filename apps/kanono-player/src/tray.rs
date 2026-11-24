use std::thread;

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
