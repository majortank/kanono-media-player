use std::{fs, path::{Path, PathBuf}, time::{Duration, UNIX_EPOCH}};

use anyhow::{bail, Context, Result};
use id3::{Tag, TagLike, Version};
use rusqlite::{params, types::Value, Connection, OptionalExtension};
use symphonia::core::{
    formats::FormatOptions,
    io::MediaSourceStream,
    meta::{MetadataOptions, StandardTagKey},
    probe::Hint,
};
use walkdir::WalkDir;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryTrack {
    pub id: i64,
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub year: Option<i32>,
    pub track_number: Option<i32>,
    pub duration: Option<Duration>,
}

#[derive(Debug, Clone, Default)]
pub struct TrackQuery {
    pub text: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub year: Option<i32>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct TagUpdate {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub year: Option<i32>,
}

pub struct LibraryDatabase {
    connection: Connection,
}

impl LibraryDatabase {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
            fs::create_dir_all(parent).with_context(|| format!("unable to create {}", parent.display()))?;
        }
        let connection = Connection::open(path)?;
        connection.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            CREATE TABLE IF NOT EXISTS tracks (
                id INTEGER PRIMARY KEY,
                path TEXT NOT NULL UNIQUE,
                title TEXT NOT NULL,
                artist TEXT NOT NULL,
                album TEXT NOT NULL,
                genre TEXT NOT NULL,
                year INTEGER,
                track_number INTEGER,
                duration_ms INTEGER,
                modified_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS tracks_artist_album ON tracks(artist, album);
            CREATE INDEX IF NOT EXISTS tracks_genre_year ON tracks(genre, year);
            ",
        )?;
        Ok(Self { connection })
    }

    /// Recursively indexes supported audio files, skipping rows unchanged since the last scan.
    pub fn scan_directory(&mut self, root: impl AsRef<Path>) -> Result<usize> {
        let root = root.as_ref();
        let transaction = self.connection.transaction()?;
        let mut indexed = 0;
        for entry in WalkDir::new(root).follow_links(false) {
            let entry = match entry { Ok(entry) => entry, Err(_) => continue };
            let path = entry.path();
            if !entry.file_type().is_file() || !is_supported(path) { continue; }
            let modified_at = modification_seconds(path)?;
            let existing: Option<i64> = transaction.query_row(
                "SELECT modified_at FROM tracks WHERE path = ?1",
                [path.to_string_lossy().as_ref()],
                |row| row.get(0),
            ).optional()?;
            if existing == Some(modified_at) { continue; }
            let track = read_track(path)?;
            transaction.execute(
                "INSERT INTO tracks (path, title, artist, album, genre, year, track_number, duration_ms, modified_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(path) DO UPDATE SET title = excluded.title, artist = excluded.artist,
                 album = excluded.album, genre = excluded.genre, year = excluded.year,
                 track_number = excluded.track_number, duration_ms = excluded.duration_ms,
                 modified_at = excluded.modified_at",
                params![
                    track.path.to_string_lossy(), track.title, track.artist, track.album, track.genre,
                    track.year, track.track_number, track.duration.map(|value| value.as_millis() as i64), modified_at,
                ],
            )?;
            indexed += 1;
        }
        transaction.commit()?;
        Ok(indexed)
    }

    pub fn query(&self, query: &TrackQuery) -> Result<Vec<LibraryTrack>> {
        let mut clauses = Vec::new();
        let mut values = Vec::<Value>::new();
        if let Some(text) = query.text.as_deref() {
            clauses.push("(title LIKE ? OR artist LIKE ? OR album LIKE ? OR genre LIKE ?)");
            let pattern = Value::Text(format!("%{text}%"));
            values.extend([pattern.clone(), pattern.clone(), pattern.clone(), pattern]);
        }
        for (column, value) in [("artist", &query.artist), ("album", &query.album), ("genre", &query.genre)] {
            if let Some(value) = value {
                clauses.push(match column { "artist" => "artist LIKE ?", "album" => "album LIKE ?", _ => "genre LIKE ?" });
                values.push(Value::Text(format!("%{value}%")));
            }
        }
        if let Some(year) = query.year {
            clauses.push("year = ?");
            values.push(Value::Integer(i64::from(year)));
        }
        let where_clause = if clauses.is_empty() { String::new() } else { format!(" WHERE {}", clauses.join(" AND ")) };
        let limit = query.limit.map(|limit| format!(" LIMIT {limit}")).unwrap_or_default();
        let sql = format!("SELECT id, path, title, artist, album, genre, year, track_number, duration_ms FROM tracks{where_clause} ORDER BY artist COLLATE NOCASE, album COLLATE NOCASE, track_number, title COLLATE NOCASE{limit}");
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(rusqlite::params_from_iter(values), row_to_track)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
    }

    /// Writes identical tags to selected MP3 files and updates their indexed fields atomically.
    pub fn mass_tag(&mut self, track_ids: &[i64], update: &TagUpdate) -> Result<()> {
        let transaction = self.connection.transaction()?;
        for track_id in track_ids {
            let track = transaction.query_row(
                "SELECT id, path, title, artist, album, genre, year, track_number, duration_ms FROM tracks WHERE id = ?1",
                [track_id],
                row_to_track,
            )?;
            if !track.path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("mp3")) {
                bail!("mass tag writing currently supports MP3 only: {}", track.path.display());
            }
            write_id3_tags(&track, update)?;
            transaction.execute(
                "UPDATE tracks SET title = ?1, artist = ?2, album = ?3, genre = ?4, year = ?5, modified_at = ?6 WHERE id = ?7",
                params![
                    update.title.as_deref().unwrap_or(&track.title), update.artist.as_deref().unwrap_or(&track.artist),
                    update.album.as_deref().unwrap_or(&track.album), update.genre.as_deref().unwrap_or(&track.genre),
                    update.year.or(track.year), modification_seconds(&track.path)?, track_id,
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }
}

fn read_track(path: &Path) -> Result<LibraryTrack> {
    let default_title = path.file_stem().and_then(|value| value.to_str()).unwrap_or("Unknown title").to_owned();
    let mut track = LibraryTrack {
        id: 0,
        path: path.to_owned(),
        title: default_title.clone(),
        artist: String::new(),
        album: String::new(),
        genre: String::new(),
        year: None,
        track_number: None,
        duration: None,
    };

    // 1. Extract metadata and duration using Symphonia format probe
    if let Ok(file) = fs::File::open(path) {
        let stream = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|ext| ext.to_str()) {
            hint.with_extension(ext);
        }
        if let Ok(mut probed) = symphonia::default::get_probe().format(
            &hint,
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        ) {
            // Duration calculation from default audio track
            if let Some(audio_track) = probed.format.default_track() {
                if let (Some(time_base), Some(n_frames)) = (audio_track.codec_params.time_base, audio_track.codec_params.n_frames) {
                    let duration = time_base.calc_time(n_frames);
                    let secs = duration.seconds as f64 + f64::from(duration.frac);
                    if secs > 0.0 {
                        track.duration = Some(Duration::from_secs_f64(secs));
                    }
                }
            }

            // Extract tags from container / stream metadata
            let mut extract_tags = |tags: &[symphonia::core::meta::Tag]| {
                for tag in tags {
                    let val = match &tag.value {
                        symphonia::core::meta::Value::String(s) => s.trim().to_string(),
                        val => val.to_string().trim().to_string(),
                    };
                    if val.is_empty() {
                        continue;
                    }
                    match tag.std_key {
                        Some(StandardTagKey::TrackTitle) => track.title = val,
                        Some(StandardTagKey::Artist) => track.artist = val,
                        Some(StandardTagKey::Album) => track.album = val,
                        Some(StandardTagKey::Genre) => track.genre = val,
                        Some(StandardTagKey::Date) | Some(StandardTagKey::ReleaseDate) => {
                            if let Ok(year) = val.chars().take(4).collect::<String>().parse::<i32>() {
                                track.year = Some(year);
                            }
                        }
                        Some(StandardTagKey::TrackNumber) => {
                            if let Ok(num) = val.split('/').next().unwrap_or("").trim().parse::<i32>() {
                                track.track_number = Some(num);
                            }
                        }
                        _ => {
                            let k = tag.key.to_ascii_uppercase();
                            if (k == "TITLE" || k == "TIT2") && (track.title.is_empty() || track.title == default_title) {
                                track.title = val;
                            } else if (k == "ARTIST" || k == "TPE1") && track.artist.is_empty() {
                                track.artist = val;
                            } else if (k == "ALBUM" || k == "TALB") && track.album.is_empty() {
                                track.album = val;
                            } else if (k == "GENRE" || k == "TCON") && track.genre.is_empty() {
                                track.genre = val;
                            }
                        }
                    }
                }
            };

            if let Some(rev) = probed.metadata.get().as_ref().and_then(|m| m.current()) {
                extract_tags(rev.tags());
            }
            if let Some(rev) = probed.format.metadata().current() {
                extract_tags(rev.tags());
            }
        }
    }

    // 2. Fallback to ID3 for MP3 files or missing tags
    if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("mp3")) || track.artist.is_empty() {
        if let Ok(tag) = Tag::read_from_path(path) {
            if let Some(title) = tag.title() {
                if !title.is_empty() {
                    track.title = title.to_owned();
                }
            }
            if let Some(artist) = tag.artist() {
                if !artist.is_empty() && track.artist.is_empty() {
                    track.artist = artist.to_owned();
                }
            }
            if let Some(album) = tag.album() {
                if !album.is_empty() && track.album.is_empty() {
                    track.album = album.to_owned();
                }
            }
            if let Some(genre) = tag.genre() {
                if !genre.is_empty() && track.genre.is_empty() {
                    track.genre = genre.to_owned();
                }
            }
            if track.year.is_none() {
                track.year = tag.year();
            }
            if track.track_number.is_none() {
                track.track_number = tag.track().map(|v| v as i32);
            }
            if track.duration.is_none() {
                if let Some(dur_secs) = tag.duration() {
                    if dur_secs > 0 {
                        track.duration = Some(Duration::from_secs(dur_secs as u64));
                    }
                }
            }
        }
    }

    if track.artist.is_empty() {
        track.artist = "Unknown Artist".to_string();
    }
    if track.album.is_empty() {
        track.album = "Unknown Album".to_string();
    }

    Ok(track)
}

fn write_id3_tags(track: &LibraryTrack, update: &TagUpdate) -> Result<()> {
    let mut tag = Tag::read_from_path(&track.path).unwrap_or_else(|_| Tag::new());
    if let Some(value) = &update.title { tag.set_title(value); }
    if let Some(value) = &update.artist { tag.set_artist(value); }
    if let Some(value) = &update.album { tag.set_album(value); }
    if let Some(value) = &update.genre { tag.set_genre(value); }
    if let Some(value) = update.year { tag.set_year(value); }
    tag.write_to_path(&track.path, Version::Id3v24).with_context(|| format!("failed to update {}", track.path.display()))
}

fn row_to_track(row: &rusqlite::Row<'_>) -> rusqlite::Result<LibraryTrack> {
    let duration_ms: Option<i64> = row.get(8)?;
    Ok(LibraryTrack {
        id: row.get(0)?, path: PathBuf::from(row.get::<_, String>(1)?), title: row.get(2)?, artist: row.get(3)?, album: row.get(4)?, genre: row.get(5)?,
        year: row.get(6)?, track_number: row.get(7)?, duration: duration_ms.map(|value| Duration::from_millis(value as u64)),
    })
}

fn is_supported(path: &Path) -> bool {
    path.extension().and_then(|extension| extension.to_str()).is_some_and(|extension| {
        matches!(
            extension.to_ascii_lowercase().as_str(),
            "mp3" | "mp2" | "mp1" | "flac" | "wav" | "wave" | "webm" | "mkv" | "ogg" | "oga" | "m4a" | "m4b" | "mp4" | "aac" | "alac" | "aiff" | "aif" | "caf"
        )
    })
}

fn modification_seconds(path: &Path) -> Result<i64> {
    Ok(fs::metadata(path)?.modified()?.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_composes_playlist_filters() {
        let database = LibraryDatabase::open(":memory:").unwrap();
        database.connection.execute(
            "INSERT INTO tracks (path, title, artist, album, genre, year, track_number, duration_ms, modified_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params!["/music/a.mp3", "Motion Picture Soundtrack", "Radiohead", "Kid A", "Alternative", 2000, 10, 260_000, 1],
        ).unwrap();
        database.connection.execute(
            "INSERT INTO tracks (path, title, artist, album, genre, year, track_number, duration_ms, modified_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params!["/music/b.mp3", "Teardrop", "Massive Attack", "Mezzanine", "Electronic", 1998, 3, 330_000, 1],
        ).unwrap();

        let tracks = database.query(&TrackQuery {
            text: Some("motion".into()),
            artist: Some("radio".into()),
            album: Some("kid".into()),
            genre: Some("alternative".into()),
            year: Some(2000),
            limit: Some(10),
        }).unwrap();

        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title, "Motion Picture Soundtrack");
    }
}