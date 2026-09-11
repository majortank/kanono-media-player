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
    pub play_count: u32,
    pub bpm: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct TrackQuery {
    pub text: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub year: Option<i32>,
    pub min_duration: Option<Duration>,
    pub max_duration: Option<Duration>,
    pub min_bpm: Option<u32>,
    pub max_bpm: Option<u32>,
    pub sort_by_play_count: bool,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct TagUpdate {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub year: Option<i32>,
    pub bpm: Option<u32>,
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
                modified_at INTEGER NOT NULL,
                play_count INTEGER DEFAULT 0,
                bpm INTEGER,
                last_played_at INTEGER
            );
            CREATE INDEX IF NOT EXISTS tracks_artist_album ON tracks(artist, album);
            CREATE INDEX IF NOT EXISTS tracks_genre_year ON tracks(genre, year);
            ",
        )?;

        // Ensure optional migration columns exist on existing databases
        let _ = connection.execute("ALTER TABLE tracks ADD COLUMN play_count INTEGER DEFAULT 0", []);
        let _ = connection.execute("ALTER TABLE tracks ADD COLUMN bpm INTEGER", []);
        let _ = connection.execute("ALTER TABLE tracks ADD COLUMN last_played_at INTEGER", []);

        // Create index on play_count after ensuring the column exists
        let _ = connection.execute("CREATE INDEX IF NOT EXISTS tracks_play_count ON tracks(play_count DESC)", []);

        Ok(Self { connection })
    }

    /// Recursively indexes supported audio files, skipping rows unchanged since the last scan.
    pub fn scan_directory(&mut self, root: impl AsRef<Path>) -> Result<usize> {
        self.scan_paths(&[root.as_ref().to_path_buf()])
    }

    /// Indexes a list of files or directories.
    pub fn scan_paths(&mut self, paths: &[PathBuf]) -> Result<usize> {
        let transaction = self.connection.transaction()?;
        let mut indexed = 0;

        for input_path in paths {
            let mut process_file = |p: &Path| -> Result<()> {
                if !is_supported(p) { return Ok(()); }
                let modified_at = modification_seconds(p)?;
                let existing: Option<i64> = transaction.query_row(
                    "SELECT modified_at FROM tracks WHERE path = ?1",
                    [p.to_string_lossy().as_ref()],
                    |row| row.get(0),
                ).optional()?;
                if existing == Some(modified_at) { return Ok(()); }
                let track = read_track(p)?;
                transaction.execute(
                    "INSERT INTO tracks (path, title, artist, album, genre, year, track_number, duration_ms, modified_at, play_count, bpm)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                     ON CONFLICT(path) DO UPDATE SET title = excluded.title, artist = excluded.artist,
                     album = excluded.album, genre = excluded.genre, year = excluded.year,
                     track_number = excluded.track_number, duration_ms = excluded.duration_ms,
                     modified_at = excluded.modified_at, bpm = excluded.bpm",
                    params![
                        track.path.to_string_lossy(), track.title, track.artist, track.album, track.genre,
                        track.year, track.track_number, track.duration.map(|value| value.as_millis() as i64), modified_at,
                        track.play_count, track.bpm,
                    ],
                )?;
                indexed += 1;
                Ok(())
            };

            if input_path.is_file() {
                let _ = process_file(input_path);
            } else if input_path.is_dir() {
                for entry in WalkDir::new(input_path).follow_links(false) {
                    let entry = match entry { Ok(entry) => entry, Err(_) => continue };
                    if entry.file_type().is_file() {
                        let _ = process_file(entry.path());
                    }
                }
            }
        }

        transaction.commit()?;
        Ok(indexed)
    }

    /// Updates exact duration for a track.
    pub fn update_duration(&mut self, track_id: i64, duration: Duration) -> Result<()> {
        self.connection.execute(
            "UPDATE tracks SET duration_ms = ?1 WHERE id = ?2",
            params![duration.as_millis() as i64, track_id],
        )?;
        Ok(())
    }

    /// Records a play for a track and returns the updated play count.
    pub fn record_play(&mut self, track_id: i64) -> Result<u32> {
        let now = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        self.connection.execute(
            "UPDATE tracks SET play_count = COALESCE(play_count, 0) + 1, last_played_at = ?1 WHERE id = ?2",
            params![now, track_id],
        )?;
        let count: u32 = self.connection.query_row(
            "SELECT COALESCE(play_count, 0) FROM tracks WHERE id = ?1",
            [track_id],
            |row| row.get(0),
        ).unwrap_or(1);
        Ok(count)
    }

    pub fn query(&self, query: &TrackQuery) -> Result<Vec<LibraryTrack>> {
        let mut clauses = Vec::new();
        let mut values = Vec::<Value>::new();
        if let Some(text) = query.text.as_deref() {
            clauses.push("(title LIKE ? OR artist LIKE ? OR album LIKE ? OR genre LIKE ? OR path LIKE ?)");
            let pattern = Value::Text(format!("%{text}%"));
            values.extend([pattern.clone(), pattern.clone(), pattern.clone(), pattern.clone(), pattern]);
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
        if let Some(min_dur) = query.min_duration {
            clauses.push("duration_ms >= ?");
            values.push(Value::Integer(min_dur.as_millis() as i64));
        }
        if let Some(max_dur) = query.max_duration {
            clauses.push("duration_ms <= ?");
            values.push(Value::Integer(max_dur.as_millis() as i64));
        }
        if let Some(min_bpm) = query.min_bpm {
            clauses.push("bpm >= ?");
            values.push(Value::Integer(i64::from(min_bpm)));
        }
        if let Some(max_bpm) = query.max_bpm {
            clauses.push("bpm <= ?");
            values.push(Value::Integer(i64::from(max_bpm)));
        }
        let where_clause = if clauses.is_empty() { String::new() } else { format!(" WHERE {}", clauses.join(" AND ")) };
        let order_by = if query.sort_by_play_count {
            "ORDER BY play_count DESC, artist COLLATE NOCASE, album COLLATE NOCASE, title COLLATE NOCASE"
        } else {
            "ORDER BY artist COLLATE NOCASE, album COLLATE NOCASE, track_number, title COLLATE NOCASE"
        };
        let limit = query.limit.map(|limit| format!(" LIMIT {limit}")).unwrap_or_default();
        let sql = format!("SELECT id, path, title, artist, album, genre, year, track_number, duration_ms, play_count, bpm FROM tracks{where_clause} {order_by}{limit}");
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(rusqlite::params_from_iter(values), row_to_track)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
    }

    /// Writes identical tags to selected files and updates their indexed fields atomically.
    pub fn mass_tag(&mut self, track_ids: &[i64], update: &TagUpdate) -> Result<()> {
        let transaction = self.connection.transaction()?;
        for &track_id in track_ids {
            let track = transaction.query_row(
                "SELECT id, path, title, artist, album, genre, year, track_number, duration_ms, play_count, bpm FROM tracks WHERE id = ?1",
                [track_id],
                row_to_track,
            )?;
            // If MP3, write ID3 tags directly to the audio file if possible
            if track.path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("mp3")) {
                let _ = write_id3_tags(&track, update);
            }
            let mod_time = modification_seconds(&track.path).unwrap_or_else(|_| {
                std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64
            });
            transaction.execute(
                "UPDATE tracks SET title = ?1, artist = ?2, album = ?3, genre = ?4, year = ?5, bpm = ?6, modified_at = ?7 WHERE id = ?8",
                params![
                    update.title.as_deref().unwrap_or(&track.title),
                    update.artist.as_deref().unwrap_or(&track.artist),
                    update.album.as_deref().unwrap_or(&track.album),
                    update.genre.as_deref().unwrap_or(&track.genre),
                    update.year.or(track.year),
                    update.bpm.or(track.bpm),
                    mod_time,
                    track_id,
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }
}

pub fn read_track(path: &Path) -> Result<LibraryTrack> {
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
        play_count: 0,
        bpm: None,
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
                    let secs = duration.seconds as f64 + duration.frac;
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
                        Some(StandardTagKey::Bpm) => {
                            if let Ok(bpm) = val.parse::<f32>() {
                                track.bpm = Some(bpm.round() as u32);
                            }
                        }
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
                            } else if (k == "BPM" || k == "TBPM") && track.bpm.is_none() {
                                if let Ok(bpm) = val.parse::<f32>() {
                                    track.bpm = Some(bpm.round() as u32);
                                }
                            } else if (k == "DURATION" || k == "LENGTH" || k == "TLEN") && track.duration.is_none() {
                                if let Some(dur) = parse_duration_string(&val) {
                                    track.duration = Some(dur);
                                }
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

    // 2. Direct WAV header duration extraction for WAV files if not already determined
    if track.duration.is_none() && path.extension().is_some_and(|e| e.eq_ignore_ascii_case("wav") || e.eq_ignore_ascii_case("wave")) {
        if let Ok(dur) = read_wav_duration(path) {
            track.duration = Some(dur);
        }
    }

    // 3. Fallback to ID3 for MP3 files or missing tags
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
            if track.bpm.is_none() {
                if let Some(bpm_frame) = tag.get("TBPM") {
                    if let Some(text) = bpm_frame.content().text() {
                        if let Ok(bpm) = text.trim().parse::<f32>() {
                            track.bpm = Some(bpm.round() as u32);
                        }
                    }
                }
            }
            if track.duration.is_none() {
                if let Some(dur_secs) = tag.duration() {
                    if dur_secs > 0 {
                        track.duration = Some(Duration::from_secs(dur_secs as u64));
                    }
                }
                if track.duration.is_none() {
                    if let Some(tlen_frame) = tag.get("TLEN") {
                        if let Some(text) = tlen_frame.content().text() {
                            if let Ok(ms) = text.trim().parse::<u64>() {
                                if ms > 0 {
                                    track.duration = Some(Duration::from_millis(ms));
                                }
                            }
                        }
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

fn read_wav_duration(path: &Path) -> Result<Duration> {
    use std::io::Read;
    let mut file = fs::File::open(path)?;
    let mut header = [0u8; 12];
    file.read_exact(&mut header)?;
    if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        bail!("not a RIFF/WAVE file");
    }
    let mut byte_rate = 0u64;
    let mut data_size = 0u64;
    let mut chunk_header = [0u8; 8];
    while file.read_exact(&mut chunk_header).is_ok() {
        let chunk_id = &chunk_header[0..4];
        let chunk_size = u32::from_le_bytes([chunk_header[4], chunk_header[5], chunk_header[6], chunk_header[7]]) as u64;
        if chunk_id == b"fmt " {
            let mut fmt_buf = vec![0u8; chunk_size as usize];
            file.read_exact(&mut fmt_buf)?;
            if fmt_buf.len() >= 16 {
                let channels = u16::from_le_bytes([fmt_buf[2], fmt_buf[3]]) as u64;
                let sample_rate = u32::from_le_bytes([fmt_buf[4], fmt_buf[5], fmt_buf[6], fmt_buf[7]]) as u64;
                let bits = u16::from_le_bytes([fmt_buf[14], fmt_buf[15]]) as u64;
                byte_rate = sample_rate * channels * (bits / 8);
            }
        } else if chunk_id == b"data" {
            data_size = chunk_size;
            break;
        } else {
            use std::io::Seek;
            file.seek(std::io::SeekFrom::Current(chunk_size as i64))?;
        }
    }
    if byte_rate > 0 && data_size > 0 {
        let secs = data_size as f64 / byte_rate as f64;
        return Ok(Duration::from_secs_f64(secs));
    }
    bail!("unable to determine WAV duration from header")
}

fn parse_duration_string(s: &str) -> Option<Duration> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if s.contains(':') {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() == 2 {
            let mins: f64 = parts[0].parse().ok()?;
            let secs: f64 = parts[1].parse().ok()?;
            return Some(Duration::from_secs_f64(mins * 60.0 + secs));
        } else if parts.len() == 3 {
            let hours: f64 = parts[0].parse().ok()?;
            let mins: f64 = parts[1].parse().ok()?;
            let secs: f64 = parts[2].parse().ok()?;
            return Some(Duration::from_secs_f64(hours * 3600.0 + mins * 60.0 + secs));
        }
    } else if let Ok(val) = s.parse::<f64>() {
        if val > 10000.0 && !s.contains('.') {
            return Some(Duration::from_millis(val as u64));
        } else if val > 0.0 {
            return Some(Duration::from_secs_f64(val));
        }
    }
    None
}

fn write_id3_tags(track: &LibraryTrack, update: &TagUpdate) -> Result<()> {
    let mut tag = Tag::read_from_path(&track.path).unwrap_or_else(|_| Tag::new());
    if let Some(value) = &update.title { tag.set_title(value); }
    if let Some(value) = &update.artist { tag.set_artist(value); }
    if let Some(value) = &update.album { tag.set_album(value); }
    if let Some(value) = &update.genre { tag.set_genre(value); }
    if let Some(value) = update.year { tag.set_year(value); }
    if let Some(value) = update.bpm {
        use id3::frame::Content;
        tag.add_frame(id3::Frame::with_content("TBPM", Content::Text(value.to_string())));
    }
    tag.write_to_path(&track.path, Version::Id3v24).with_context(|| format!("failed to update {}", track.path.display()))
}

fn row_to_track(row: &rusqlite::Row<'_>) -> rusqlite::Result<LibraryTrack> {
    let duration_ms: Option<i64> = row.get(8)?;
    let play_count: u32 = row.get::<_, Option<u32>>(9)?.unwrap_or(0);
    let bpm: Option<u32> = row.get(10)?;
    Ok(LibraryTrack {
        id: row.get(0)?,
        path: PathBuf::from(row.get::<_, String>(1)?),
        title: row.get(2)?,
        artist: row.get(3)?,
        album: row.get(4)?,
        genre: row.get(5)?,
        year: row.get(6)?,
        track_number: row.get(7)?,
        duration: duration_ms.map(|value| Duration::from_millis(value as u64)),
        play_count,
        bpm,
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
        let mut database = LibraryDatabase::open(":memory:").unwrap();
        database.connection.execute(
            "INSERT INTO tracks (path, title, artist, album, genre, year, track_number, duration_ms, modified_at, play_count, bpm)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params!["/music/a.mp3", "Motion Picture Soundtrack", "Radiohead", "Kid A", "Alternative", 2000, 10, 260_000, 1, 5, 120],
        ).unwrap();
        database.connection.execute(
            "INSERT INTO tracks (path, title, artist, album, genre, year, track_number, duration_ms, modified_at, play_count, bpm)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params!["/music/b.mp3", "Teardrop", "Massive Attack", "Mezzanine", "Electronic", 1998, 3, 330_000, 1, 2, 90],
        ).unwrap();

        let tracks = database.query(&TrackQuery {
            text: Some("motion".into()),
            artist: Some("radio".into()),
            album: Some("kid".into()),
            genre: Some("alternative".into()),
            year: Some(2000),
            limit: Some(10),
            ..Default::default()
        }).unwrap();

        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title, "Motion Picture Soundtrack");
        assert_eq!(tracks[0].play_count, 5);
        assert_eq!(tracks[0].bpm, Some(120));

        // Test play count recording
        let new_count = database.record_play(tracks[0].id).unwrap();
        assert_eq!(new_count, 6);

        // Test duration update
        database.update_duration(tracks[0].id, Duration::from_secs(265)).unwrap();
        let updated = database.query(&TrackQuery {
            artist: Some("Radiohead".into()),
            limit: Some(1),
            ..Default::default()
        }).unwrap();
        assert_eq!(updated[0].duration, Some(Duration::from_secs(265)));

        // Test sort by play count
        let most_played = database.query(&TrackQuery {
            sort_by_play_count: true,
            ..Default::default()
        }).unwrap();
        assert_eq!(most_played[0].title, "Motion Picture Soundtrack");
        assert_eq!(most_played[0].play_count, 6);
        assert_eq!(most_played[1].title, "Teardrop");
        assert_eq!(most_played[1].play_count, 2);

        // Test filter by duration range
        let dur_filtered = database.query(&TrackQuery {
            min_duration: Some(Duration::from_secs(300)),
            ..Default::default()
        }).unwrap();
        assert_eq!(dur_filtered.len(), 1);
        assert_eq!(dur_filtered[0].title, "Teardrop");

        // Test filter by BPM range
        let bpm_filtered = database.query(&TrackQuery {
            min_bpm: Some(100),
            ..Default::default()
        }).unwrap();
        assert_eq!(bpm_filtered.len(), 1);
        assert_eq!(bpm_filtered[0].title, "Motion Picture Soundtrack");
    }
}