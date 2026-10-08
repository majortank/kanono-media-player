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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryFolder {
    pub id: i64,
    pub path: PathBuf,
    pub added_at: i64,
    pub last_scanned_at: Option<i64>,
    pub track_count: usize,
    pub exists_on_disk: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LibraryStats {
    pub total_tracks: usize,
    pub total_artists: usize,
    pub total_albums: usize,
    pub total_genres: usize,
    pub total_duration: Duration,
    pub total_folders: usize,
    pub total_plays: u64,
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

/// Cleans and sanitizes metadata strings by replacing null bytes (\0) with commas,
/// stripping control characters, and normalizing whitespace.
/// This prevents crashes in UI toolkits and Wayland protocols (NulError) caused by null characters.
pub fn sanitize_metadata_string(s: &str) -> String {
    let parts: Vec<&str> = s
        .split('\0')
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .collect();

    if parts.is_empty() {
        return String::new();
    }

    let joined = parts.join(", ");
    let cleaned: String = joined
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();

    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

impl LibraryDatabase {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
            fs::create_dir_all(parent).with_context(|| format!("unable to create database directory {}", parent.display()))?;
        }
        let mut connection = Connection::open(path)
            .with_context(|| format!("unable to open database file: {}", path.display()))?;
        let _ = connection.busy_timeout(Duration::from_secs(5));
        if connection.execute_batch("PRAGMA journal_mode = WAL;").is_err() {
            let _ = connection.execute_batch("PRAGMA journal_mode = DELETE;");
        }
        let _ = connection.execute_batch("PRAGMA synchronous = NORMAL;");
        connection.execute_batch(
            "
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

            CREATE TABLE IF NOT EXISTS library_folders (
                id INTEGER PRIMARY KEY,
                path TEXT NOT NULL UNIQUE,
                added_at INTEGER NOT NULL,
                last_scanned_at INTEGER
            );
            ",
        )?;

        // Ensure optional migration columns exist on existing databases
        let _ = connection.execute("ALTER TABLE tracks ADD COLUMN play_count INTEGER DEFAULT 0", []);
        let _ = connection.execute("ALTER TABLE tracks ADD COLUMN bpm INTEGER", []);
        let _ = connection.execute("ALTER TABLE tracks ADD COLUMN last_played_at INTEGER", []);

        // Create index on play_count after ensuring the column exists
        let _ = connection.execute("CREATE INDEX IF NOT EXISTS tracks_play_count ON tracks(play_count DESC)", []);

        // Sanitize any existing rows containing embedded null bytes in title, artist, album, genre
        let dirty_rows: Vec<(i64, String, String, String, String)> = {
            let mut sanitize_stmt = connection.prepare(
                "SELECT id, title, artist, album, genre FROM tracks
                 WHERE instr(title, char(0)) > 0
                    OR instr(artist, char(0)) > 0
                    OR instr(album, char(0)) > 0
                    OR instr(genre, char(0)) > 0",
            )?;
            let rows = sanitize_stmt
                .query_map([], |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                })?
                .filter_map(|r| r.ok())
                .collect();
            rows
        };

        if !dirty_rows.is_empty() {
            let tx = connection.transaction()?;
            for (id, title, artist, album, genre) in dirty_rows {
                let clean_title = sanitize_metadata_string(&title);
                let clean_artist = sanitize_metadata_string(&artist);
                let clean_album = sanitize_metadata_string(&album);
                let clean_genre = sanitize_metadata_string(&genre);
                tx.execute(
                    "UPDATE tracks SET title = ?1, artist = ?2, album = ?3, genre = ?4 WHERE id = ?5",
                    params![clean_title, clean_artist, clean_album, clean_genre, id],
                )?;
            }
            tx.commit()?;
        }

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

        let mut check_stmt = transaction.prepare("SELECT modified_at FROM tracks WHERE path = ?1")?;
        let mut insert_stmt = transaction.prepare(
            "INSERT INTO tracks (path, title, artist, album, genre, year, track_number, duration_ms, modified_at, play_count, bpm)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(path) DO UPDATE SET title = excluded.title, artist = excluded.artist,
             album = excluded.album, genre = excluded.genre, year = excluded.year,
             track_number = excluded.track_number, duration_ms = excluded.duration_ms,
             modified_at = excluded.modified_at, bpm = excluded.bpm",
        )?;

        for input_path in paths {
            let mut process_file = |p: &Path| -> Result<()> {
                if !is_supported(p) { return Ok(()); }
                let modified_at = modification_seconds(p)?;
                let path_str = p.to_string_lossy();
                let existing: Option<i64> = check_stmt.query_row(
                    [path_str.as_ref()],
                    |row| row.get(0),
                ).optional()?;
                if existing == Some(modified_at) { return Ok(()); }
                let track = read_track(p)?;
                insert_stmt.execute(
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

        drop(check_stmt);
        drop(insert_stmt);
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

    /// Registers a folder in the managed library folders list.
    pub fn add_folder(&mut self, folder: &Path) -> Result<i64> {
        let path_str = folder.to_string_lossy();
        let now = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        self.connection.execute(
            "INSERT INTO library_folders (path, added_at, last_scanned_at)
             VALUES (?1, ?2, NULL)
             ON CONFLICT(path) DO NOTHING",
            params![path_str, now],
        )?;
        let id: i64 = self.connection.query_row(
            "SELECT id FROM library_folders WHERE path = ?1",
            [path_str.as_ref()],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    /// Registers a folder and scans it immediately.
    pub fn add_folder_and_scan(&mut self, folder: &Path) -> Result<usize> {
        let folder_id = self.add_folder(folder)?;
        let count = self.scan_directory(folder)?;
        let _ = self.update_folder_scanned(folder_id);
        Ok(count)
    }

    /// Removes a folder from monitored folders, optionally removing its tracks.
    pub fn remove_folder(&mut self, folder_id: i64, remove_tracks: bool) -> Result<usize> {
        let folder_path: Option<String> = self.connection.query_row(
            "SELECT path FROM library_folders WHERE id = ?1",
            [folder_id],
            |row| row.get(0),
        ).optional()?;

        let mut tracks_deleted = 0;
        if let Some(folder_path) = folder_path {
            if remove_tracks {
                let prefix_pattern = format!("{}%", folder_path);
                tracks_deleted = self.connection.execute(
                    "DELETE FROM tracks WHERE path LIKE ?1",
                    params![prefix_pattern],
                )?;
            }
            self.connection.execute(
                "DELETE FROM library_folders WHERE id = ?1",
                params![folder_id],
            )?;
        }
        Ok(tracks_deleted)
    }

    /// Returns raw monitored folder records without running N+1 queries.
    pub fn get_raw_folders(&self) -> Result<Vec<(i64, PathBuf, i64, Option<i64>)>> {
        let mut stmt = self.connection.prepare(
            "SELECT id, path, added_at, last_scanned_at FROM library_folders ORDER BY path COLLATE NOCASE ASC"
        )?;
        let rows = stmt.query_map([], |row| {
            let id: i64 = row.get(0)?;
            let path_str: String = row.get(1)?;
            let added_at: i64 = row.get(2)?;
            let last_scanned_at: Option<i64> = row.get(3)?;
            Ok((id, PathBuf::from(path_str), added_at, last_scanned_at))
        })?;
        let mut folders = Vec::new();
        for item in rows {
            folders.push(item?);
        }
        Ok(folders)
    }

    /// Returns all monitored folders with live track count and disk existence check.
    pub fn get_folders(&self) -> Result<Vec<LibraryFolder>> {
        let raw = self.get_raw_folders()?;
        let mut count_stmt = self.connection.prepare(
            "SELECT COUNT(*) FROM tracks WHERE path LIKE ?1"
        )?;

        let mut folders = Vec::with_capacity(raw.len());
        for (id, path, added_at, last_scanned_at) in raw {
            let exists_on_disk = path.exists();
            let prefix_pattern = format!("{}%", path.to_string_lossy());
            let track_count: usize = count_stmt.query_row(
                params![prefix_pattern],
                |r| r.get(0),
            ).unwrap_or(0);

            folders.push(LibraryFolder {
                id,
                path,
                added_at,
                last_scanned_at,
                track_count,
                exists_on_disk,
            });
        }
        Ok(folders)
    }

    /// Updates the last scanned timestamp for a folder.
    pub fn update_folder_scanned(&mut self, folder_id: i64) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        self.connection.execute(
            "UPDATE library_folders SET last_scanned_at = ?1 WHERE id = ?2",
            params![now, folder_id],
        )?;
        Ok(())
    }

    /// Rescans all registered monitored folders.
    pub fn rescan_all_folders(&mut self) -> Result<usize> {
        let folders = self.get_folders()?;
        let mut total_indexed = 0;
        for f in folders {
            if f.exists_on_disk {
                if let Ok(c) = self.scan_directory(&f.path) {
                    total_indexed += c;
                    let _ = self.update_folder_scanned(f.id);
                }
            }
        }
        Ok(total_indexed)
    }

    /// Deletes a track from the library.
    pub fn delete_track(&mut self, track_id: i64) -> Result<bool> {
        let affected = self.connection.execute(
            "DELETE FROM tracks WHERE id = ?1",
            params![track_id],
        )?;
        Ok(affected > 0)
    }

    /// Deletes multiple tracks from the library atomically.
    pub fn delete_tracks(&mut self, track_ids: &[i64]) -> Result<usize> {
        if track_ids.is_empty() {
            return Ok(0);
        }
        let tx = self.connection.transaction()?;
        let mut deleted = 0;
        for &id in track_ids {
            deleted += tx.execute("DELETE FROM tracks WHERE id = ?1", params![id])?;
        }
        tx.commit()?;
        Ok(deleted)
    }

    /// Checks all library tracks against the filesystem and removes any whose files no longer exist.
    pub fn prune_missing_tracks(&mut self) -> Result<usize> {
        let missing_ids: Vec<i64> = {
            let mut stmt = self.connection.prepare("SELECT id, path FROM tracks")?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?;

            let mut ids = Vec::new();
            for r in rows {
                let (id, path_str) = r?;
                if !Path::new(&path_str).exists() {
                    ids.push(id);
                }
            }
            ids
        };

        if missing_ids.is_empty() {
            return Ok(0);
        }

        self.delete_tracks(&missing_ids)
    }

    /// Clears all tracks and monitored folders from the database.
    pub fn clear_library(&mut self) -> Result<()> {
        let tx = self.connection.transaction()?;
        tx.execute("DELETE FROM tracks", [])?;
        tx.execute("DELETE FROM library_folders", [])?;
        tx.commit()?;
        Ok(())
    }

    /// Computes summary statistics for the library.
    pub fn get_library_stats(&self) -> Result<LibraryStats> {
        let total_tracks: usize = self.connection.query_row(
            "SELECT COUNT(*) FROM tracks",
            [],
            |r| r.get(0),
        ).unwrap_or(0);

        let total_artists: usize = self.connection.query_row(
            "SELECT COUNT(DISTINCT artist) FROM tracks WHERE artist != '' AND artist != 'Unknown Artist'",
            [],
            |r| r.get(0),
        ).unwrap_or(0);

        let total_albums: usize = self.connection.query_row(
            "SELECT COUNT(DISTINCT album) FROM tracks WHERE album != '' AND album != 'Unknown Album'",
            [],
            |r| r.get(0),
        ).unwrap_or(0);

        let total_genres: usize = self.connection.query_row(
            "SELECT COUNT(DISTINCT genre) FROM tracks WHERE genre != ''",
            [],
            |r| r.get(0),
        ).unwrap_or(0);

        let total_duration_ms: i64 = self.connection.query_row(
            "SELECT COALESCE(SUM(duration_ms), 0) FROM tracks",
            [],
            |r| r.get(0),
        ).unwrap_or(0);

        let total_folders: usize = self.connection.query_row(
            "SELECT COUNT(*) FROM library_folders",
            [],
            |r| r.get(0),
        ).unwrap_or(0);

        let total_plays: u64 = self.connection.query_row(
            "SELECT COALESCE(SUM(play_count), 0) FROM tracks",
            [],
            |r| r.get(0),
        ).unwrap_or(0);

        Ok(LibraryStats {
            total_tracks,
            total_artists,
            total_albums,
            total_genres,
            total_duration: Duration::from_millis(total_duration_ms.max(0) as u64),
            total_folders,
            total_plays,
        })
    }
}

/// Computes summary statistics for a slice of tracks in memory in < 1 millisecond.
pub fn compute_library_stats(tracks: &[LibraryTrack], total_folders: usize) -> LibraryStats {
    let mut artists = std::collections::HashSet::new();
    let mut albums = std::collections::HashSet::new();
    let mut genres = std::collections::HashSet::new();
    let mut total_duration = Duration::ZERO;
    let mut total_plays = 0u64;

    for track in tracks {
        if !track.artist.is_empty() && track.artist != "Unknown Artist" {
            artists.insert(&track.artist);
        }
        if !track.album.is_empty() && track.album != "Unknown Album" {
            albums.insert(&track.album);
        }
        if !track.genre.is_empty() {
            genres.insert(&track.genre);
        }
        if let Some(dur) = track.duration {
            total_duration += dur;
        }
        total_plays += u64::from(track.play_count);
    }

    LibraryStats {
        total_tracks: tracks.len(),
        total_artists: artists.len(),
        total_albums: albums.len(),
        total_genres: genres.len(),
        total_duration,
        total_folders,
        total_plays,
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
            // Duration calculation from any track with valid frames
            for audio_track in probed.format.tracks() {
                if let (Some(time_base), Some(n_frames)) = (audio_track.codec_params.time_base, audio_track.codec_params.n_frames) {
                    let duration = time_base.calc_time(n_frames);
                    let secs = duration.seconds as f64 + duration.frac;
                    if secs > 0.0 {
                        track.duration = Some(Duration::from_secs_f64(secs));
                        break;
                    }
                }
            }

            // Extract tags from container / stream metadata
            let mut extract_tags = |tags: &[symphonia::core::meta::Tag]| {
                for tag in tags {
                    let raw = match &tag.value {
                        symphonia::core::meta::Value::String(s) => s.as_str(),
                        _ => "",
                    };
                    let val = if raw.is_empty() {
                        sanitize_metadata_string(&tag.value.to_string())
                    } else {
                        sanitize_metadata_string(raw)
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
                let clean = sanitize_metadata_string(title);
                if !clean.is_empty() {
                    track.title = clean;
                }
            }
            if let Some(artist) = tag.artist() {
                let clean = sanitize_metadata_string(artist);
                if !clean.is_empty() && track.artist.is_empty() {
                    track.artist = clean;
                }
            }
            if let Some(album) = tag.album() {
                let clean = sanitize_metadata_string(album);
                if !clean.is_empty() && track.album.is_empty() {
                    track.album = clean;
                }
            }
            if let Some(genre) = tag.genre() {
                let clean = sanitize_metadata_string(genre);
                if !clean.is_empty() && track.genre.is_empty() {
                    track.genre = clean;
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

    // 4. Fallback to ffprobe for container/video formats or missing duration
    let is_video_or_container = path.extension().and_then(|ext| ext.to_str()).is_some_and(|ext| {
        matches!(
            ext.to_ascii_lowercase().as_str(),
            "webm" | "mkv" | "mp4" | "m4v" | "avi" | "mov" | "wmv" | "flv" | "3gp" | "ts" | "mts" | "m2ts" | "ogv" | "vob" | "asf"
        )
    });
    let needs_probe = track.duration.is_none() || (is_video_or_container && (track.artist.is_empty() || track.artist == "Unknown Artist"));
    if needs_probe && crate::ffmpeg::is_ffmpeg_available() {
        if let Some(media_meta) = crate::ffmpeg::probe_file(path) {
            if track.duration.is_none() {
                track.duration = media_meta.duration;
            }
            if let Some(title) = media_meta.title {
                let clean = sanitize_metadata_string(&title);
                if !clean.is_empty() && (track.title.is_empty() || track.title == default_title) {
                    track.title = clean;
                }
            }
            if let Some(artist) = media_meta.artist {
                let clean = sanitize_metadata_string(&artist);
                if !clean.is_empty() && track.artist.is_empty() {
                    track.artist = clean;
                }
            }
            if let Some(album) = media_meta.album {
                let clean = sanitize_metadata_string(&album);
                if !clean.is_empty() && track.album.is_empty() {
                    track.album = clean;
                }
            }
            if let Some(genre) = media_meta.genre {
                let clean = sanitize_metadata_string(&genre);
                if !clean.is_empty() && track.genre.is_empty() {
                    track.genre = clean;
                }
            }
            if track.year.is_none() {
                track.year = media_meta.year;
            }
            if track.track_number.is_none() {
                track.track_number = media_meta.track_number;
            }
        }
    }

    track.title = sanitize_metadata_string(&track.title);
    track.artist = sanitize_metadata_string(&track.artist);
    track.album = sanitize_metadata_string(&track.album);
    track.genre = sanitize_metadata_string(&track.genre);

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
        title: sanitize_metadata_string(&row.get::<_, String>(2)?),
        artist: sanitize_metadata_string(&row.get::<_, String>(3)?),
        album: sanitize_metadata_string(&row.get::<_, String>(4)?),
        genre: sanitize_metadata_string(&row.get::<_, String>(5)?),
        year: row.get(6)?,
        track_number: row.get(7)?,
        duration: duration_ms.map(|value| Duration::from_millis(value as u64)),
        play_count,
        bpm,
    })
}

pub fn is_supported_audio_extension(path: &Path) -> bool {
    path.extension().and_then(|extension| extension.to_str()).is_some_and(|extension| {
        matches!(
            extension.to_ascii_lowercase().as_str(),
            // Dedicated audio formats
            "mp3" | "mp2" | "mp1" | "flac" | "wav" | "wave" | "ogg" | "oga" | "m4a" | "m4b" | "aac" | "alac" | "aiff" | "aif" | "caf"
            // Video formats containing music / audio
            | "webm" | "mkv" | "mp4" | "m4v" | "avi" | "mov" | "wmv" | "flv" | "3gp" | "ts" | "mts" | "m2ts" | "ogv" | "vob" | "asf"
        )
    })
}

pub fn is_supported(path: &Path) -> bool {
    is_supported_audio_extension(path)
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

    #[test]
    fn test_video_track_reading_and_indexing() {
        // Test supported extension check
        assert!(is_supported_audio_extension(Path::new("song.mp4")));
        assert!(is_supported_audio_extension(Path::new("video.mkv")));
        assert!(is_supported_audio_extension(Path::new("clip.webm")));
        assert!(is_supported_audio_extension(Path::new("movie.avi")));
        assert!(is_supported_audio_extension(Path::new("music.mov")));
        assert!(is_supported_audio_extension(Path::new("track.flv")));
        assert!(is_supported_audio_extension(Path::new("song.wmv")));

        let test_user_path = Path::new("/home/likanono/Downloads/Bekezela_1080p.mp4");
        if test_user_path.exists() {
            let track = read_track(test_user_path).expect("read_track should succeed on MP4 video song");
            assert!(!track.title.is_empty());
            assert!(track.duration.is_some(), "Duration must be resolved for MP4 song");
            let dur = track.duration.unwrap();
            assert!(dur.as_secs() > 400, "Bekezela duration should be ~451s");
        }
    }

    #[test]
    fn test_sanitize_metadata_string() {
        assert_eq!(
            sanitize_metadata_string("Khopolo Ka Lejoe La Ferene 4\0KHOPOLO"),
            "Khopolo Ka Lejoe La Ferene 4, KHOPOLO"
        );
        assert_eq!(sanitize_metadata_string("Artist\0"), "Artist");
        assert_eq!(sanitize_metadata_string("\0\0Artist\0\0"), "Artist");
        assert_eq!(sanitize_metadata_string("Ben Acker\0Ben Blacker"), "Ben Acker, Ben Blacker");
        assert_eq!(sanitize_metadata_string("Track 1\r\n"), "Track 1");
        assert_eq!(sanitize_metadata_string(""), "");
    }

    #[test]
    fn test_database_null_character_migration() {
        let temp_dir = std::env::temp_dir().join(format!("kanono_test_db_{}", std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        let db_path = temp_dir.join("test_library.sqlite3");
        let _ = fs::create_dir_all(&temp_dir);

        {
            // Create dirty database directly with embedded null characters
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "CREATE TABLE tracks (
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
                );",
            ).unwrap();
            conn.execute(
                "INSERT INTO tracks (id, path, title, artist, album, genre, modified_at)
                 VALUES (1, '/p/1.mp3', 'Track 1', ?1, ?2, 'Genre', 100)",
                params!["Khopolo Ka Lejoe La Ferene 4\0KHOPOLO", "Album\0Name"],
            ).unwrap();
        }

        {
            // Opening LibraryDatabase should automatically migrate and sanitize all fields
            let db = LibraryDatabase::open(&db_path).unwrap();
            let tracks = db.query(&TrackQuery::default()).unwrap();
            assert_eq!(tracks.len(), 1);
            assert_eq!(tracks[0].artist, "Khopolo Ka Lejoe La Ferene 4, KHOPOLO");
            assert_eq!(tracks[0].album, "Album, Name");
            assert!(!tracks[0].artist.contains('\0'));
            assert!(!tracks[0].album.contains('\0'));
        }

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_library_manager_operations() {
        let mut db = LibraryDatabase::open(":memory:").unwrap();

        // 1. Add monitored folders
        let folder_id_1 = db.add_folder(Path::new("/music/rock")).unwrap();
        let folder_id_2 = db.add_folder(Path::new("/music/jazz")).unwrap();
        assert!(folder_id_1 > 0);
        assert!(folder_id_2 > 0);

        // Duplicate folder add should be idempotent
        let dup_id = db.add_folder(Path::new("/music/rock")).unwrap();
        assert_eq!(dup_id, folder_id_1);

        // 2. Add tracks
        db.connection.execute(
            "INSERT INTO tracks (id, path, title, artist, album, genre, year, track_number, duration_ms, modified_at, play_count)
             VALUES (1, '/music/rock/song1.mp3', 'Song 1', 'Queen', 'A Night at the Opera', 'Rock', 1975, 1, 300_000, 100, 10),
                    (2, '/music/rock/song2.mp3', 'Song 2', 'Queen', 'A Night at the Opera', 'Rock', 1975, 2, 240_000, 100, 5),
                    (3, '/music/jazz/tune1.flac', 'Tune 1', 'Miles Davis', 'Kind of Blue', 'Jazz', 1959, 1, 540_000, 100, 2)",
            [],
        ).unwrap();

        // 3. Check get_folders and track counting
        let folders = db.get_folders().unwrap();
        assert_eq!(folders.len(), 2);
        let rock_f = folders.iter().find(|f| f.path == PathBuf::from("/music/rock")).unwrap();
        assert_eq!(rock_f.track_count, 2);
        let jazz_f = folders.iter().find(|f| f.path == PathBuf::from("/music/jazz")).unwrap();
        assert_eq!(jazz_f.track_count, 1);

        // 4. Check library stats and in-memory computation
        let stats = db.get_library_stats().unwrap();
        assert_eq!(stats.total_tracks, 3);
        assert_eq!(stats.total_artists, 2);
        assert_eq!(stats.total_albums, 2);
        assert_eq!(stats.total_genres, 2);
        assert_eq!(stats.total_duration, Duration::from_millis(1080_000));
        assert_eq!(stats.total_folders, 2);
        assert_eq!(stats.total_plays, 17);

        // Verify get_raw_folders and compute_library_stats match DB stats exactly
        let raw_folders = db.get_raw_folders().unwrap();
        assert_eq!(raw_folders.len(), 2);
        let all_tracks = db.query(&TrackQuery::default()).unwrap();
        let in_memory_stats = compute_library_stats(&all_tracks, raw_folders.len());
        assert_eq!(in_memory_stats, stats);

        // 5. Delete single track
        assert!(db.delete_track(1).unwrap());
        assert_eq!(db.query(&TrackQuery::default()).unwrap().len(), 2);

        // 6. Delete multiple tracks
        let deleted = db.delete_tracks(&[2]).unwrap();
        assert_eq!(deleted, 1);
        assert_eq!(db.query(&TrackQuery::default()).unwrap().len(), 1);

        // 7. Remove folder and its remaining tracks
        let deleted_tracks = db.remove_folder(folder_id_2, true).unwrap();
        assert_eq!(deleted_tracks, 1);
        assert_eq!(db.query(&TrackQuery::default()).unwrap().len(), 0);
        assert_eq!(db.get_folders().unwrap().len(), 1);

        // 8. Clear library
        db.clear_library().unwrap();
        assert_eq!(db.get_folders().unwrap().len(), 0);
        assert_eq!(db.get_library_stats().unwrap().total_tracks, 0);
    }
}