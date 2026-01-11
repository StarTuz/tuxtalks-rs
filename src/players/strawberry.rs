use anyhow::{Context, Result};
use async_trait::async_trait;
use rusqlite::Connection;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use zbus::Connection as ZBusConnection;

use crate::players::mpris_utils::MprisPlayerProxy;
use crate::players::MediaPlayer;

pub struct StrawberryPlayer {
    db_path: PathBuf,
}

impl StrawberryPlayer {
    pub fn new(db_path: PathBuf) -> Self {
        Self { db_path }
    }

    async fn get_proxy(conn: &ZBusConnection) -> Result<MprisPlayerProxy<'_>> {
        let proxy = MprisPlayerProxy::builder(conn)
            .destination("org.mpris.MediaPlayer2.strawberry")?
            .build()
            .await?;
        Ok(proxy)
    }

    fn query_db<P, F, T>(&self, query: &str, params: P, row_fn: F) -> Result<Vec<T>>
    where
        P: rusqlite::Params,
        F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
    {
        if !self.db_path.exists() {
            let fallback = dirs::home_dir()
                .context("No home dir")?
                .join(".local/share/strawberry/strawberry/strawberry.db");
            if fallback.exists() {
                let conn = Connection::open(fallback)?;
                let mut stmt = conn.prepare(query)?;
                let rows = stmt.query_map(params, row_fn)?;
                let mut results = Vec::new();
                for row in rows {
                    results.push(row?);
                }
                return Ok(results);
            }
            anyhow::bail!("Strawberry database not found at {:?}", self.db_path);
        }

        let conn = Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare(query)?;
        let rows = stmt.query_map(params, row_fn)?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    fn play_files(&self, files: Vec<String>) -> Result<()> {
        if files.is_empty() {
            anyhow::bail!("No files to play");
        }

        let mut cmd = Command::new("strawberry");
        cmd.arg("--load");
        for f in files {
            if !f.starts_with("file://") {
                let path = std::path::Path::new(&f);
                if path.is_absolute() {
                    cmd.arg(format!("file://{}", f));
                } else {
                    cmd.arg(f);
                }
            } else {
                cmd.arg(f);
            }
        }

        cmd.spawn()?;
        Ok(())
    }
}

#[async_trait]
impl MediaPlayer for StrawberryPlayer {
    async fn play_genre(&self, genre: &str) -> Result<()> {
        let query = "SELECT url FROM songs WHERE genre LIKE ? ORDER BY random() LIMIT 100";
        let files = self.query_db(query, [format!("%{}%", genre)], |row| {
            row.get::<_, String>(0)
        })?;
        self.play_files(files)?;
        Ok(())
    }

    async fn play_random(&self) -> Result<()> {
        let query = "SELECT url FROM songs ORDER BY random() LIMIT 100";
        let files = self.query_db(query, [], |row| row.get::<_, String>(0))?;
        self.play_files(files)?;
        Ok(())
    }

    async fn play_artist(&self, artist: &str) -> Result<()> {
        let query = "SELECT url FROM songs WHERE artist LIKE ? ORDER BY album, track, disc";
        let files = self.query_db(query, [format!("%{}%", artist)], |row| {
            row.get::<_, String>(0)
        })?;
        self.play_files(files)?;
        Ok(())
    }

    async fn play_album(&self, album: &str) -> Result<()> {
        let query = "SELECT url FROM songs WHERE album LIKE ? ORDER BY track, disc";
        let files = self.query_db(query, [format!("%{}%", album)], |row| {
            row.get::<_, String>(0)
        })?;
        self.play_files(files)?;
        Ok(())
    }

    async fn play_song(&self, song: &str) -> Result<()> {
        let query = "SELECT url FROM songs WHERE title LIKE ? ORDER BY artist, album";
        let files = self.query_db(query, [format!("%{}%", song)], |row| {
            row.get::<_, String>(0)
        })?;
        self.play_files(files)?;
        Ok(())
    }

    async fn play_playlist(&self, playlist: &str, shuffle: bool) -> Result<()> {
        let query = "SELECT rowid FROM playlists WHERE name LIKE ? LIMIT 1";
        let rowid: Vec<i64> =
            self.query_db(query, [format!("%{}%", playlist)], |row| row.get(0))?;

        if let Some(id) = rowid.first() {
            let track_query = if shuffle {
                "SELECT url FROM playlist_items WHERE playlist_id = ? ORDER BY random()"
            } else {
                "SELECT url FROM playlist_items WHERE playlist_id = ? ORDER BY rowid"
            };
            let files = self.query_db(track_query, [*id], |row| row.get::<_, String>(0))?;
            self.play_files(files)?;
            Ok(())
        } else {
            anyhow::bail!("Playlist not found: {}", playlist)
        }
    }

    async fn play_pause(&self) -> Result<()> {
        let conn = ZBusConnection::session().await?;
        let proxy = Self::get_proxy(&conn).await?;
        proxy.play_pause().await?;
        Ok(())
    }

    async fn next_track(&self) -> Result<()> {
        let conn = ZBusConnection::session().await?;
        let proxy = Self::get_proxy(&conn).await?;
        proxy.next().await?;
        Ok(())
    }

    async fn previous_track(&self) -> Result<()> {
        let conn = ZBusConnection::session().await?;
        let proxy = Self::get_proxy(&conn).await?;
        proxy.previous().await?;
        Ok(())
    }

    async fn volume_up(&self) -> Result<()> {
        let conn = ZBusConnection::session().await?;
        let proxy = Self::get_proxy(&conn).await?;
        let current = proxy.volume().await?;
        proxy.set_volume((current + 0.1).min(1.0)).await?;
        Ok(())
    }

    async fn volume_down(&self) -> Result<()> {
        let conn = ZBusConnection::session().await?;
        let proxy = Self::get_proxy(&conn).await?;
        let current = proxy.volume().await?;
        proxy.set_volume((current - 0.1).max(0.0)).await?;
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        let conn = ZBusConnection::session().await?;
        let proxy = Self::get_proxy(&conn).await?;
        proxy.stop().await?;
        Ok(())
    }

    async fn what_is_playing(&self) -> Result<String> {
        let conn = ZBusConnection::session().await?;
        let proxy = Self::get_proxy(&conn).await?;
        let metadata = proxy.metadata().await?;

        let title = metadata
            .get("xesam:title")
            .and_then(|v| {
                if let zbus::zvariant::Value::Str(s) = v {
                    Some(s.to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "Unknown Title".to_string());

        let artist = metadata
            .get("xesam:artist")
            .and_then(|v| {
                if let zbus::zvariant::Value::Array(arr) = v {
                    arr.iter().next().and_then(|v| {
                        if let zbus::zvariant::Value::Str(s) = v {
                            Some(s.to_string())
                        } else {
                            None
                        }
                    })
                } else if let zbus::zvariant::Value::Str(s) = v {
                    Some(s.to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "Unknown Artist".to_string());

        Ok(format!("{} by {}", title, artist))
    }

    async fn play_any(&self, query: &str) -> Result<Vec<crate::players::SearchResult>> {
        use crate::players::{SearchResult, SearchResultType};
        use crate::utils::fuzzy::find_matches;

        let mut candidates: Vec<SearchResult> = Vec::new();

        // Get all artists from database
        let artists: Vec<String> = self
            .query_db(
                "SELECT DISTINCT artist FROM songs WHERE artist IS NOT NULL AND artist != ''",
                [],
                |row| row.get(0),
            )
            .unwrap_or_default();

        for m in find_matches(query, &artists, 5, 0.6) {
            candidates.push(SearchResult {
                display: format!("Artist: {}", m.value),
                value: m.value,
                result_type: SearchResultType::Artist,
                score: m.score,
            });
        }

        // Get all albums from database
        let albums: Vec<String> = self
            .query_db(
                "SELECT DISTINCT album FROM songs WHERE album IS NOT NULL AND album != ''",
                [],
                |row| row.get(0),
            )
            .unwrap_or_default();

        for m in find_matches(query, &albums, 5, 0.6) {
            candidates.push(SearchResult {
                display: format!("Album: {}", m.value),
                value: m.value,
                result_type: SearchResultType::Album,
                score: m.score,
            });
        }

        // Get playlists
        let playlists: Vec<String> = self
            .query_db(
                "SELECT name FROM playlists WHERE name IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .unwrap_or_default();

        for m in find_matches(query, &playlists, 5, 0.6) {
            candidates.push(SearchResult {
                display: format!("Playlist: {}", m.value),
                value: m.value,
                result_type: SearchResultType::Playlist,
                score: m.score,
            });
        }

        // Sort by score
        candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates.truncate(10);

        Ok(candidates)
    }

    async fn get_all_artists(&self, limit: usize) -> Vec<String> {
        self.query_db(
            &format!("SELECT DISTINCT artist FROM songs WHERE artist IS NOT NULL AND artist != '' LIMIT {}", limit),
            [],
            |row| row.get(0)
        ).unwrap_or_default()
    }

    async fn list_tracks(&self) -> Vec<(String, String)> {
        // Get current album from MPRIS metadata
        let conn = match zbus::Connection::session().await {
            Ok(c) => c,
            Err(_) => return vec![],
        };

        let proxy = match Self::get_proxy(&conn).await {
            Ok(p) => p,
            Err(_) => return vec![],
        };

        let metadata = match proxy.metadata().await {
            Ok(m) => m,
            Err(_) => return vec![],
        };

        // Extract album from metadata
        let album = metadata.get("xesam:album").and_then(|v| {
            if let zbus::zvariant::Value::Str(s) = v {
                Some(s.to_string())
            } else {
                None
            }
        });

        if let Some(album_name) = album {
            // Query database for tracks in this album
            self.query_db(
                "SELECT title, url FROM songs WHERE album = ? ORDER BY track, disc",
                [&album_name],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .unwrap_or_default()
        } else {
            vec![]
        }
    }

    async fn health_check(&self) -> bool {
        let conn = match ZBusConnection::session().await {
            Ok(c) => c,
            Err(_) => return false,
        };

        match conn
            .call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus"),
                "NameHasOwner",
                &"org.mpris.MediaPlayer2.strawberry",
            )
            .await
        {
            Ok(reply) => {
                let has_owner: bool = reply.body().deserialize().unwrap_or(false);
                if has_owner {
                    return true;
                }
            }
            Err(_) => return false,
        }

        let _ = Command::new("strawberry")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();

        for _ in 0..10 {
            tokio::time::sleep(Duration::from_secs(1)).await;
            if let Ok(reply) = conn
                .call_method(
                    Some("org.freedesktop.DBus"),
                    "/org/freedesktop/DBus",
                    Some("org.freedesktop.DBus"),
                    "NameHasOwner",
                    &"org.mpris.MediaPlayer2.strawberry",
                )
                .await
            {
                if reply.body().deserialize().unwrap_or(false) {
                    return true;
                }
            }
        }

        false
    }
}
