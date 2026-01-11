use anyhow::{Context, Result};
use async_trait::async_trait;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use zbus::Connection as ZBusConnection;

use crate::library::LocalLibrary;
use crate::players::mpris_utils::MprisPlayerProxy;
use crate::players::MediaPlayer;

pub struct MprisPlayer {
    service_name: String,
    library: Arc<LocalLibrary>,
}

impl MprisPlayer {
    pub fn new(service_name: String, library: Arc<LocalLibrary>) -> Self {
        Self {
            service_name,
            library,
        }
    }

    async fn get_proxy<'a>(
        conn: &'a ZBusConnection,
        service_name: &'a str,
    ) -> Result<MprisPlayerProxy<'a>> {
        let proxy = MprisPlayerProxy::builder(conn)
            .destination(service_name)?
            .build()
            .await?;
        Ok(proxy)
    }

    async fn play_files_async(&self, files: Vec<String>) -> Result<()> {
        if files.is_empty() {
            anyhow::bail!("No files to play");
        }

        let uri = if files.len() > 1 {
            let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
            let temp_path = std::env::temp_dir().join(format!("tuxtalks_playlist_{}.m3u8", now));

            use std::io::Write;
            let mut f = std::fs::File::create(&temp_path)?;
            writeln!(f, "#EXTM3U")?;
            for path in files {
                writeln!(f, "{}", path)?;
            }
            format!(
                "file://{}",
                temp_path.to_str().context("Invalid temp path")?
            )
        } else {
            format!("file://{}", files[0])
        };

        let conn = ZBusConnection::session().await?;
        let proxy = Self::get_proxy(&conn, &self.service_name).await?;
        proxy.open_uri(&uri).await?;
        Ok(())
    }
}

#[async_trait]
impl MediaPlayer for MprisPlayer {
    async fn play_genre(&self, genre: &str) -> Result<()> {
        let tracks = self.library.search_tracks(genre)?;
        let files: Vec<String> = tracks
            .into_iter()
            .filter(|t| t.genre.to_lowercase().contains(&genre.to_lowercase()))
            .map(|t| t.path)
            .collect();
        self.play_files_async(files).await?;
        Ok(())
    }

    async fn play_random(&self) -> Result<()> {
        let tracks = self.library.get_random_tracks(50)?;
        let files: Vec<String> = tracks.into_iter().map(|t| t.path).collect();
        self.play_files_async(files).await?;
        Ok(())
    }

    async fn play_artist(&self, artist: &str) -> Result<()> {
        let tracks = self.library.search_tracks(artist)?;
        let files: Vec<String> = tracks
            .into_iter()
            .filter(|t| t.artist.to_lowercase().contains(&artist.to_lowercase()))
            .map(|t| t.path)
            .collect();
        self.play_files_async(files).await?;
        Ok(())
    }

    async fn play_album(&self, album: &str) -> Result<()> {
        let files = self.library.get_album_tracks(album)?;
        self.play_files_async(files).await?;
        Ok(())
    }

    async fn play_song(&self, song: &str) -> Result<()> {
        let tracks = self.library.search_tracks(song)?;
        let files: Vec<String> = tracks
            .into_iter()
            .filter(|t| t.title.to_lowercase().contains(&song.to_lowercase()))
            .map(|t| t.path)
            .collect();
        self.play_files_async(files).await?;
        Ok(())
    }

    async fn play_playlist(&self, playlist: &str, _shuffle: bool) -> Result<()> {
        let playlists = self.library.search_playlists(playlist)?;
        if let Some((_, path)) = playlists.first() {
            self.play_files_async(vec![path.clone()]).await?;
            Ok(())
        } else {
            anyhow::bail!("Playlist not found: {}", playlist)
        }
    }

    async fn play_pause(&self) -> Result<()> {
        let conn = ZBusConnection::session().await?;
        let proxy = Self::get_proxy(&conn, &self.service_name).await?;
        proxy.play_pause().await?;
        Ok(())
    }

    async fn next_track(&self) -> Result<()> {
        let conn = ZBusConnection::session().await?;
        let proxy = Self::get_proxy(&conn, &self.service_name).await?;
        proxy.next().await?;
        Ok(())
    }

    async fn previous_track(&self) -> Result<()> {
        let conn = ZBusConnection::session().await?;
        let proxy = Self::get_proxy(&conn, &self.service_name).await?;
        proxy.previous().await?;
        Ok(())
    }

    async fn volume_up(&self) -> Result<()> {
        let conn = ZBusConnection::session().await?;
        let proxy = Self::get_proxy(&conn, &self.service_name).await?;
        let current = proxy.volume().await?;
        proxy.set_volume((current + 0.1).min(1.0)).await?;
        Ok(())
    }

    async fn volume_down(&self) -> Result<()> {
        let conn = ZBusConnection::session().await?;
        let proxy = Self::get_proxy(&conn, &self.service_name).await?;
        let current = proxy.volume().await?;
        proxy.set_volume((current - 0.1).max(0.0)).await?;
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        let conn = ZBusConnection::session().await?;
        let proxy = Self::get_proxy(&conn, &self.service_name).await?;
        proxy.stop().await?;
        Ok(())
    }

    async fn what_is_playing(&self) -> Result<String> {
        let conn = ZBusConnection::session().await?;
        let proxy = Self::get_proxy(&conn, &self.service_name).await?;
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
                &self.service_name,
            )
            .await
        {
            Ok(reply) => reply.body().deserialize().unwrap_or(false),
            Err(_) => false,
        }
    }
}
