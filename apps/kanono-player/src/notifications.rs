use notify_rust::{Notification, Timeout};

pub fn track_started(title: &str, artist: &str, album: &str) {
    let body = if artist.is_empty() && album.is_empty() {
        title.to_string()
    } else if album.is_empty() {
        format!("{artist}\n{title}")
    } else {
        format!("{artist}\n{title}\n{album}")
    };

    if let Err(error) = Notification::new()
        .appname("Kanono Media Player")
        .summary("Now playing")
        .body(&body)
        .icon("kanono-media-player")
        .timeout(Timeout::Milliseconds(4000))
        .show()
    {
        eprintln!("unable to show track notification: {error}");
    }
}

pub fn error(summary: &str, body: &str) {
    if let Err(error) = Notification::new()
        .appname("Kanono Media Player")
        .summary(summary)
        .body(body)
        .icon("kanono-media-player")
        .timeout(Timeout::Milliseconds(5000))
        .show()
    {
        eprintln!("unable to show notification: {error}");
    }
}
