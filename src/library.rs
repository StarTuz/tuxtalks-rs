use anyhow::Result;
use lofty::prelude::*;
use lofty::probe::Probe;
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct LocalLibrary {
    db_path: PathBuf,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Track {
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub composer: String,
    pub genre: String,
    pub track_number: u32,
    pub media_type: String,
}

impl LocalLibrary {
    pub fn new(db_path: PathBuf) -> Result<Self> {
        let library = Self { db_path };
        library.init_db()?;
        Ok(library)
    }

    fn init_db(&self) -> Result<()> {
        if let Some(parent) = self.db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&self.db_path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tracks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT UNIQUE,
                title TEXT,
                artist TEXT,
                album TEXT,
                composer TEXT,
                genre TEXT,
                track_number INTEGER,
                media_type TEXT
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS playlists (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT UNIQUE,
                name TEXT
            )",
            [],
        )?;
        Ok(())
    }

    pub fn scan_directory(&self, root_path: &Path) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        conn.execute("DELETE FROM tracks", [])?;
        conn.execute("DELETE FROM playlists", [])?;

        let audio_exts = ["mp3", "flac", "ogg", "m4a", "wav"];
        let video_exts = ["mp4", "mkv", "avi", "mov", "webm", "mpg", "mpeg"];
        let playlist_exts = ["m3u", "m3u8", "pls"];

        for entry in WalkDir::new(root_path).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                let path = entry.path();
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();

                if audio_exts.contains(&ext.as_str()) || video_exts.contains(&ext.as_str()) {
                    let media_type = if audio_exts.contains(&ext.as_str()) {
                        "audio"
                    } else {
                        "video"
                    };
                    let metadata = self.read_metadata(path);

                    let _ = conn.execute(
                        "INSERT OR REPLACE INTO tracks (path, title, artist, album, composer, genre, track_number, media_type)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                        (
                            path.to_str().unwrap_or(""),
                            metadata.title.unwrap_or_else(|| path.file_stem().and_then(|s| s.to_str()).unwrap_or("Unknown").to_string()),
                            metadata.artist.unwrap_or_else(|| "Unknown Artist".to_string()),
                            metadata.album.unwrap_or_else(|| "Unknown Album".to_string()),
                            metadata.composer.unwrap_or_default(),
                            metadata.genre.unwrap_or_default(),
                            metadata.track_number.unwrap_or(0),
                            media_type,
                        ),
                    );
                } else if playlist_exts.contains(&ext.as_str()) {
                    let name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("Unknown Playlist");
                    let _ = conn.execute(
                        "INSERT OR REPLACE INTO playlists (path, name) VALUES (?, ?)",
                        (path.to_str().unwrap_or(""), name),
                    );
                }
            }
        }

        Ok(())
    }

    fn read_metadata(&self, path: &Path) -> Metadata {
        let mut meta = Metadata::default();
        if let Ok(tagged_file) = Probe::open(path).and_then(|p| p.read()) {
            if let Some(tag) = tagged_file
                .primary_tag()
                .or_else(|| tagged_file.first_tag())
            {
                meta.title = tag
                    .get_string(&lofty::tag::ItemKey::TrackTitle)
                    .map(|s| s.to_string());
                meta.artist = tag
                    .get_string(&lofty::tag::ItemKey::TrackArtist)
                    .map(|s| s.to_string());
                meta.album = tag
                    .get_string(&lofty::tag::ItemKey::AlbumTitle)
                    .map(|s| s.to_string());
                meta.genre = tag
                    .get_string(&lofty::tag::ItemKey::Genre)
                    .map(|s| s.to_string());
                // For track number, we can use the accessor if available or get_string and parse
                // But lofty tags usually have a 'track' accessor if they implement Accessor trait.
                // However, different tag formats might have different ways.
                // Let's use get_string and parse or just leave it for now if complex.
                // Actually, tag.get_string is safer for a generic approach.
            }
        }
        meta
    }

    pub fn search_tracks(&self, query: &str) -> Result<Vec<Track>> {
        let conn = Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT path, title, artist, album, composer, genre, track_number, media_type 
             FROM tracks 
             WHERE title LIKE ? OR artist LIKE ? OR album LIKE ? OR composer LIKE ?",
        )?;
        let q = format!("%{}%", query);
        let rows = stmt.query_map([&q, &q, &q, &q], |row| {
            Ok(Track {
                path: row.get(0)?,
                title: row.get(1)?,
                artist: row.get(2)?,
                album: row.get(3)?,
                composer: row.get(4)?,
                genre: row.get(5)?,
                track_number: row.get(6)?,
                media_type: row.get(7)?,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    pub fn get_artist_albums(&self, artist_query: &str) -> Result<Vec<String>> {
        let conn = Connection::open(&self.db_path)?;
        let mut stmt =
            conn.prepare("SELECT DISTINCT album FROM tracks WHERE artist LIKE ? ORDER BY album")?;
        let rows = stmt.query_map([format!("%{}%", artist_query)], |row| {
            row.get::<_, String>(0)
        })?;
        let mut albums = Vec::new();
        for row in rows {
            albums.push(row?);
        }
        Ok(albums)
    }

    pub fn get_album_tracks(&self, album: &str) -> Result<Vec<String>> {
        let conn = Connection::open(&self.db_path)?;
        let mut stmt =
            conn.prepare("SELECT path FROM tracks WHERE album = ? ORDER BY track_number, title")?;
        let rows = stmt.query_map([album], |row| row.get::<_, String>(0))?;
        let mut paths = Vec::new();
        for row in rows {
            paths.push(row?);
        }
        Ok(paths)
    }

    pub fn search_playlists(&self, query: &str) -> Result<Vec<(String, String)>> {
        let conn = Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare("SELECT name, path FROM playlists WHERE name LIKE ?")?;
        let rows = stmt.query_map([format!("%{}%", query)], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut playlists = Vec::new();
        for row in rows {
            playlists.push(row?);
        }
        Ok(playlists)
    }

    pub fn get_random_tracks(&self, limit: u32) -> Result<Vec<Track>> {
        let conn = Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT path, title, artist, album, composer, genre, track_number, media_type 
             FROM tracks 
             ORDER BY RANDOM() LIMIT ?",
        )?;
        let rows = stmt.query_map([limit], |row| {
            Ok(Track {
                path: row.get(0)?,
                title: row.get(1)?,
                artist: row.get(2)?,
                album: row.get(3)?,
                composer: row.get(4)?,
                genre: row.get(5)?,
                track_number: row.get(6)?,
                media_type: row.get(7)?,
            })
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// Check if an artist exists (Wendy Chisholm requirement)
    pub fn artist_exists(&self, artist: &str) -> bool {
        if let Ok(conn) = Connection::open(&self.db_path) {
            let stmt = conn
                .prepare("SELECT 1 FROM tracks WHERE artist = ? LIMIT 1")
                .ok();
            stmt.map(|mut s| s.exists([artist]).unwrap_or(false))
                .unwrap_or(false)
        } else {
            false
        }
    }

    /// Check if an album exists
    pub fn album_exists(&self, album: &str) -> bool {
        if let Ok(conn) = Connection::open(&self.db_path) {
            let stmt = conn
                .prepare("SELECT 1 FROM tracks WHERE album = ? LIMIT 1")
                .ok();
            stmt.map(|mut s| s.exists([album]).unwrap_or(false))
                .unwrap_or(false)
        } else {
            false
        }
    }

    /// Check if a song/title exists
    pub fn song_exists(&self, title: &str) -> bool {
        if let Ok(conn) = Connection::open(&self.db_path) {
            let stmt = conn
                .prepare("SELECT 1 FROM tracks WHERE title = ? LIMIT 1")
                .ok();
            stmt.map(|mut s| s.exists([title]).unwrap_or(false))
                .unwrap_or(false)
        } else {
            false
        }
    }

    /// Check if a playlist exists
    pub fn playlist_exists(&self, name: &str) -> bool {
        if let Ok(conn) = Connection::open(&self.db_path) {
            let stmt = conn
                .prepare("SELECT 1 FROM playlists WHERE name = ? LIMIT 1")
                .ok();
            stmt.map(|mut s| s.exists([name]).unwrap_or(false))
                .unwrap_or(false)
        } else {
            false
        }
    }
}

#[derive(Default)]
struct Metadata {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    composer: Option<String>,
    genre: Option<String>,
    track_number: Option<u32>,
}
