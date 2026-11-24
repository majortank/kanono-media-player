use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::Duration,
};

use anyhow::{anyhow, Result};
use kanono_audio_engine::{read_track, sanitize_metadata_string, LibraryTrack};

#[derive(Debug, Clone)]
pub struct YouTubeItem {
    pub id: String,
    pub title: String,
    pub channel: String,
    pub duration: Option<Duration>,
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct YouTubeConversion {
    pub path: PathBuf,
    pub track: LibraryTrack,
}

pub fn search_or_resolve(query: &str) -> Result<Vec<YouTubeItem>> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let target = if is_youtube_url(query) {
        query.to_owned()
    } else {
        format!("ytsearch5:{query}")
    };

    let output = run_yt_dlp([
        "--no-warnings",
        "--no-playlist",
        "--flat-playlist",
        "--print",
        "%(id)s\t%(title)s\t%(uploader,channel)s\t%(duration)s\t%(webpage_url)s",
        target.as_str(),
    ])?;
    if !output.status.success() {
        return Err(command_error("YouTube search failed", &output));
    }

    let mut results = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let fields: Vec<_> = line.splitn(5, '\t').collect();
        if fields.len() < 5 || fields[0].trim().is_empty() || fields[1].trim().is_empty() {
            continue;
        }
        let id = fields[0].trim().to_owned();
        let url = if fields[4].trim().is_empty() {
            format!("https://www.youtube.com/watch?v={id}")
        } else {
            fields[4].trim().to_owned()
        };
        results.push(YouTubeItem {
            id,
            title: sanitize_metadata_string(fields[1]),
            channel: fallback_channel(fields[2]),
            duration: parse_duration(fields[3]),
            url,
        });
    }

    if results.is_empty() {
        return Err(anyhow::anyhow!(
            "yt-dlp returned no playable YouTube results"
        ));
    }
    Ok(results)
}

pub fn prepare_stream_cache(item: &YouTubeItem) -> Result<LibraryTrack> {
    let cache_dir = std::env::temp_dir().join("kanono_stream_cache");
    fs::create_dir_all(&cache_dir)?;
    let template = cache_dir.join("%(id)s.%(ext)s");
    let template = template.to_string_lossy();
    let output = run_yt_dlp([
        "--no-warnings",
        "--no-progress",
        "--no-playlist",
        "-f",
        "bestaudio/best",
        "-x",
        "--audio-format",
        "mp3",
        "--audio-quality",
        "0",
        "--embed-metadata",
        "-o",
        template.as_ref(),
        "--print",
        "after_move:filepath",
        item.url.as_str(),
    ])?;
    let path = locate_output(&output, &cache_dir)?;
    let mut track = track_from_ffprobe(&path)?;
    track.id = -((item
        .id
        .bytes()
        .fold(0_i64, |hash, byte| {
            hash.wrapping_mul(31).wrapping_add(i64::from(byte))
        })
        .abs())
    .max(1));
    if track.title.is_empty() {
        track.title = item.title.clone();
    }
    let channel = if track.artist.is_empty() {
        &item.channel
    } else {
        &track.artist
    };
    track.artist = format!("{channel} (Streamed)");
    if track.album.is_empty() {
        track.album = "YouTube Stream".to_owned();
    }
    track.path = path;
    Ok(track)
}

pub fn convert_to_mp3(item: &YouTubeItem, target_dir: &Path) -> Result<YouTubeConversion> {
    fs::create_dir_all(target_dir)?;
    let template = target_dir.join("%(title)s.%(ext)s");
    let template = template.to_string_lossy();
    let output = run_yt_dlp([
        "--no-warnings",
        "--no-progress",
        "--no-playlist",
        "-f",
        "bestaudio/best",
        "-x",
        "--audio-format",
        "mp3",
        "--audio-quality",
        "0",
        "--embed-metadata",
        "-o",
        template.as_ref(),
        "--print",
        "after_move:filepath",
        item.url.as_str(),
    ])?;
    let path = locate_output(&output, target_dir)?;
    let track = track_from_ffprobe(&path)?;
    Ok(YouTubeConversion { path, track })
}

fn track_from_ffprobe(path: &Path) -> Result<LibraryTrack> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration:format_tags=title,artist,album",
            "-of",
            "default=noprint_wrappers=1",
        ])
        .arg(path)
        .output()
        .map_err(|error| {
            anyhow!("ffprobe is required to read converted audio metadata: {error}")
        })?;
    if !output.status.success() {
        return Err(command_error(
            "ffprobe could not read converted audio",
            &output,
        ));
    }

    let mut title = String::new();
    let mut artist = String::new();
    let mut album = String::new();
    let mut duration = None;
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if let Some(value) = line.strip_prefix("TAG:title=") {
            title = sanitize_metadata_string(value);
        } else if let Some(value) = line.strip_prefix("TAG:artist=") {
            artist = sanitize_metadata_string(value);
        } else if let Some(value) = line.strip_prefix("TAG:album=") {
            album = sanitize_metadata_string(value);
        } else if let Some(value) = line.strip_prefix("duration=") {
            duration = value
                .parse::<f64>()
                .ok()
                .filter(|seconds| *seconds > 0.0)
                .map(Duration::from_secs_f64);
        }
    }

    let mut track = read_track(path)
        .map_err(|error| anyhow!("converted MP3 metadata could not be read: {error}"))?;
    if !title.is_empty() {
        track.title = title;
    }
    if !artist.is_empty() {
        track.artist = artist;
    }
    if !album.is_empty() {
        track.album = album;
    }
    if duration.is_some() {
        track.duration = duration;
    }
    Ok(track)
}

fn run_yt_dlp<const N: usize>(args: [&str; N]) -> Result<Output> {
    Command::new("yt-dlp").args(args).output().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            anyhow!("yt-dlp is not installed or is not available on PATH")
        } else {
            anyhow!("unable to run yt-dlp: {error}")
        }
    })
}

fn locate_output(output: &Output, directory: &Path) -> Result<PathBuf> {
    if !output.status.success() {
        return Err(command_error("yt-dlp download failed", output));
    }
    for line in String::from_utf8_lossy(&output.stdout).lines().rev() {
        let candidate = PathBuf::from(line.trim());
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    let mut newest = None;
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.extension().is_some_and(|extension| extension == "mp3") {
            newest = Some(path);
        }
    }
    newest.ok_or_else(|| anyhow!("yt-dlp completed but no MP3 file was produced"))
}

fn command_error(prefix: &str, output: &Output) -> anyhow::Error {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let detail = stderr.trim();
    if detail.is_empty() {
        anyhow!("{prefix} (exit status {})", output.status)
    } else {
        anyhow!("{prefix}: {detail}")
    }
}

fn fallback_channel(channel: &str) -> String {
    let channel = sanitize_metadata_string(channel);
    if channel.is_empty() {
        "Unknown channel".to_owned()
    } else {
        channel
    }
}

fn parse_duration(value: &str) -> Option<Duration> {
    value
        .parse::<f64>()
        .ok()
        .filter(|seconds| *seconds > 0.0)
        .map(Duration::from_secs_f64)
}

fn is_youtube_url(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    value.starts_with("https://youtube.com/")
        || value.starts_with("http://youtube.com/")
        || value.contains("://www.youtube.com/")
        || value.contains("://m.youtube.com/")
        || value.starts_with("https://youtu.be/")
        || value.starts_with("http://youtu.be/")
}
