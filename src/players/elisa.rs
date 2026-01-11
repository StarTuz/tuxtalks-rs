use anyhow::{Context, Result};
use async_trait::async_trait;
use rusqlite::Connection;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use zbus::Connection as ZBusConnection;

use crate::players::mpris_utils::MprisPlayerProxy;
use crate::players::MediaPlayer;

pub struct ElisaPlayer {
    db_path: PathBuf,
}

impl ElisaPlayer {
    pub fn new(db_path: PathBuf) -> Self {
        Self { db_path }
    }

    async fn get_proxy(conn: &ZBusConnection) -> Result<MprisPlayerProxy<'_>> {
        let proxy = MprisPlayerProxy::builder(conn)
            .destination("org.mpris.MediaPlayer2.elisa")?
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
            // Fallback to default location
            let fallback = dirs::home_dir()
                .context("No home dir")?
                .join(".local/share/elisa/elisaDatabase.db");
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
            anyhow::bail!("Elisa database not found at {:?}", self.db_path);
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

        let _ = Command::new("elisa").args(files).spawn()?;
        Ok(())
    }
}

#[async_trait]
impl MediaPlayer for ElisaPlayer {
    async fn play_genre(&self, genre: &str) -> Result<()> {
        let query = "SELECT FileName FROM Tracks WHERE Genre LIKE ? ORDER BY random() LIMIT 100";
        let files = self.query_db(query, [format!("%{}%", genre)], |row| {
            row.get::<_, String>(0)
        })?;
        self.play_files(files)?;
        Ok(())
    }

    async fn play_random(&self) -> Result<()> {
        let query = "SELECT FileName FROM Tracks ORDER BY random() LIMIT 100";
        let files = self.query_db(query, [], |row| row.get::<_, String>(0))?;
        self.play_files(files)?;
        Ok(())
    }

    async fn play_artist(&self, artist: &str) -> Result<()> {
        let query = "SELECT FileName FROM Tracks WHERE ArtistName LIKE ? OR AlbumArtistName LIKE ? ORDER BY AlbumTitle, TrackNumber";
        let files = self.query_db(
            query,
            [format!("%{}%", artist), format!("%{}%", artist)],
            |row| row.get::<_, String>(0),
        )?;
        self.play_files(files)?;
        Ok(())
    }

    async fn play_album(&self, album: &str) -> Result<()> {
        let query =
            "SELECT FileName FROM Tracks WHERE AlbumTitle LIKE ? ORDER BY DiscNumber, TrackNumber";
        let files = self.query_db(query, [format!("%{}%", album)], |row| {
            row.get::<_, String>(0)
        })?;
        self.play_files(files)?;
        Ok(())
    }

    async fn play_song(&self, song: &str) -> Result<()> {
        let query =
            "SELECT FileName FROM Tracks WHERE Title LIKE ? ORDER BY ArtistName, AlbumTitle";
        let files = self.query_db(query, [format!("%{}%", song)], |row| {
            row.get::<_, String>(0)
        })?;
        self.play_files(files)?;
        Ok(())
    }

    async fn play_playlist(&self, _playlist: &str, _shuffle: bool) -> Result<()> {
        anyhow::bail!("Playlist support not yet implemented for Elisa")
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
                &"org.mpris.MediaPlayer2.elisa",
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

        // Attempt to launch
        let _ = Command::new("elisa").spawn();

        // Wait for it to appear
        for _ in 0..10 {
            tokio::time::sleep(Duration::from_secs(1)).await;
            if let Ok(reply) = conn
                .call_method(
                    Some("org.freedesktop.DBus"),
                    "/org/freedesktop/DBus",
                    Some("org.freedesktop.DBus"),
                    "NameHasOwner",
                    &"org.mpris.MediaPlayer2.elisa",
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
