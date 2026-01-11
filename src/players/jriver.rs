use crate::players::{MediaPlayer, SearchResult, SearchResultType};
use crate::utils::fuzzy::{find_matches, FuzzyMatch};
use anyhow::Result;
use async_trait::async_trait;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use reqwest::Client;
use std::collections::HashMap;
use std::sync::RwLock;
use tracing::{debug, info, warn};

/// JRiver Media Center player via MCWS API
pub struct JRiverPlayer {
    client: Client,
    access_key: String,
    url: String,
    /// Field value cache (Artist, Album, Genre, etc.)
    cache: RwLock<HashMap<String, Vec<String>>>,
}

impl JRiverPlayer {
    pub fn new(config: &crate::config::Config) -> Self {
        let url = format!(
            "http://{}:{}/MCWS/v1/",
            config.jriver_ip, config.jriver_port
        );
        Self {
            client: Client::new(),
            access_key: config.access_key.clone(),
            url,
            cache: RwLock::new(HashMap::new()),
        }
    }

    async fn send_command(&self, path: &str, params: &str) -> Result<String> {
        let mut url = format!(
            "{}{}?Zone=-1&ZoneType=ID&Key={}",
            self.url, path, self.access_key
        );
        if !params.is_empty() {
            url.push('&');
            url.push_str(params);
        }

        debug!("MCWS Request: {}", url);

        // Retry logic with backoff (P1.3 - Jaana requirement)
        let max_retries = 3;
        for attempt in 0..max_retries {
            match self.client.get(&url).send().await {
                Ok(resp) => {
                    return Ok(resp.text().await?);
                }
                Err(e) if attempt < max_retries - 1 => {
                    warn!(
                        "⚠️ MCWS retry {}/{} for '{}': {}",
                        attempt + 1,
                        max_retries,
                        path,
                        e
                    );

                    // If connection refused, JRiver might not be running
                    if e.is_connect() {
                        debug!("📡 Connection refused. JRiver might not be running.");
                    }

                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
                Err(e) => {
                    if e.is_connect() {
                        return Err(anyhow::anyhow!(
                            "Could not reach JRiver at {}. Is it running?",
                            self.url
                        ));
                    }
                    return Err(e.into());
                }
            }
        }

        anyhow::bail!("MCWS request failed after {} retries", max_retries)
    }

    /// Fetch all values for a field from JRiver and cache them
    async fn get_all_values(&self, field: &str) -> Vec<String> {
        // Check cache first
        {
            let cache = self.cache.read().unwrap();
            if let Some(values) = cache.get(field) {
                return values.clone();
            }
        }

        info!("📥 Fetching all {}s from library...", field);
        let params = format!("Field={}&Limit=10000", field);

        match self.send_command("Library/Values", &params).await {
            Ok(xml) => {
                let values = self.parse_library_values(&xml);
                info!("   -> Cached {} {}s.", values.len(), field);

                // Store in cache
                {
                    let mut cache = self.cache.write().unwrap();
                    cache.insert(field.to_string(), values.clone());
                }

                values
            }
            Err(e) => {
                warn!("Failed to fetch {} values: {}", field, e);
                vec![]
            }
        }
    }

    fn parse_library_values(&self, xml: &str) -> Vec<String> {
        let mut values = Vec::new();
        let mut reader = Reader::from_str(xml);

        loop {
            match reader.read_event() {
                Ok(Event::Start(ref e)) if e.name().as_ref() == b"Item" => {
                    // Read until end of Item
                }
                Ok(Event::Text(ref e)) => {
                    if let Ok(text) = e.unescape() {
                        let s = text.to_string();
                        if !s.is_empty() {
                            values.push(s);
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => (),
            }
        }

        values
    }

    /// Find matches for a search term in a specific field
    async fn find_field_matches(
        &self,
        search_term: &str,
        field: &str,
        n: usize,
    ) -> Vec<FuzzyMatch> {
        let all_values = self.get_all_values(field).await;
        find_matches(search_term, &all_values, n, 0.6)
    }

    /// Wrapper for play_playlist (Python Parity)
    pub async fn play_smartlist(&self, name: &str) -> Result<()> {
        self.play_playlist(name, false).await
    }

    /// Normalize spoken text to match library conventions (P0.2 - Wendy requirement)
    /// Handles classical music terms like "symphony", "opus", "number"
    fn normalize_text(text: &str) -> String {
        let mut result = text.to_lowercase();

        let replacements = [
            // Number words → abbreviations
            ("number one", "no. 1"),
            ("number two", "no. 2"),
            ("number three", "no. 3"),
            ("number four", "no. 4"),
            ("number five", "no. 5"),
            ("number six", "no. 6"),
            ("number seven", "no. 7"),
            ("number eight", "no. 8"),
            ("number nine", "no. 9"),
            ("number 1", "no. 1"),
            ("number 2", "no. 2"),
            ("number 3", "no. 3"),
            ("number 4", "no. 4"),
            ("number 5", "no. 5"),
            ("number 6", "no. 6"),
            ("number 7", "no. 7"),
            ("number 8", "no. 8"),
            ("number 9", "no. 9"),
            // Common ASR mishearings
            ("simply", "symphony"),
            // Opus variations
            (" opus ", " op. "),
            (" op ", " op. "),
        ];

        for (from, to) in replacements {
            result = result.replace(from, to);
        }

        result
    }

    /// Fetch all playlists
    async fn get_playlists(&self) -> Vec<(String, String)> {
        match self.send_command("Playlists/List", "").await {
            Ok(xml) => self.parse_playlists(&xml),
            Err(_) => vec![],
        }
    }

    fn parse_playlists(&self, xml: &str) -> Vec<(String, String)> {
        let mut playlists = Vec::new();
        let mut reader = Reader::from_str(xml);
        let mut current_name = String::new();
        let mut current_id = String::new();
        let mut in_field = false;
        let mut field_name = String::new();

        loop {
            match reader.read_event() {
                Ok(Event::Start(ref e)) if e.name().as_ref() == b"Field" => {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"Name" {
                            field_name = String::from_utf8_lossy(attr.value.as_ref()).to_string();
                            in_field = true;
                        }
                    }
                }
                Ok(Event::Text(ref e)) if in_field => {
                    if let Ok(text) = e.unescape() {
                        if field_name == "Name" {
                            current_name = text.to_string();
                        } else if field_name == "ID" {
                            current_id = text.to_string();
                        }
                    }
                }
                Ok(Event::End(ref e)) if e.name().as_ref() == b"Field" => {
                    in_field = false;
                }
                Ok(Event::End(ref e)) if e.name().as_ref() == b"Item" => {
                    if !current_name.is_empty() && !current_id.is_empty() {
                        playlists.push((current_name.clone(), current_id.clone()));
                    }
                    current_name.clear();
                    current_id.clear();
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => (),
            }
        }

        playlists
    }

    // ============ P2 Internal Helpers (Python Parity) ============

    /// Helper to parse PlayingNowPosition and PlayingNowTracks from Playback/Info
    fn parse_playback_position(xml: &str) -> Result<(usize, usize)> {
        let mut reader = Reader::from_str(xml);
        let mut current_pos = 0usize;
        let mut total_tracks = 0usize;
        let mut current_item = String::new();

        loop {
            match reader.read_event() {
                Ok(Event::Start(ref e)) if e.name().as_ref() == b"Item" => {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"Name" {
                            current_item = String::from_utf8_lossy(attr.value.as_ref()).to_string();
                        }
                    }
                }
                Ok(Event::Text(ref e)) => {
                    if let Ok(val) = e.unescape() {
                        if current_item == "PlayingNowPosition" {
                            current_pos = val.parse().unwrap_or(0);
                        } else if current_item == "PlayingNowTracks" {
                            total_tracks = val.parse().unwrap_or(0);
                        }
                    }
                }
                Ok(Event::End(ref e)) if e.name().as_ref() == b"Item" => {
                    current_item.clear();
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => (),
            }
        }

        Ok((current_pos, total_tracks))
    }

    /// Navigate to a specific track number in Playing Now (P2)
    pub async fn go_to_track(&self, track_number: usize) -> Result<String> {
        let xml = self.send_command("Playback/Info", "").await?;

        // Parse current position and total tracks
        let (current_pos, total_tracks) = Self::parse_playback_position(&xml)?;

        if total_tracks == 0 {
            return Ok("The playlist is still loading.".to_string());
        }

        if track_number < 1 || track_number > total_tracks {
            return Ok(format!(
                "Track {} is out of range. There are {} tracks.",
                track_number, total_tracks
            ));
        }

        let target_pos = track_number - 1;
        let offset = target_pos as i32 - current_pos as i32;

        if offset == 0 {
            return Ok(format!("Already on track {}.", track_number));
        }

        // Navigate using next/previous
        if offset > 0 {
            for _ in 0..offset {
                self.send_command("Playback/Next", "").await?;
            }
        } else {
            for _ in 0..offset.abs() {
                self.send_command("Playback/Previous", "").await?;
            }
        }

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let track_info = self.what_is_playing().await?;
        Ok(format!("Now on track {}: {}", track_number, track_info))
    }

    /// Play using PlayDoctor with a seed (P2)
    pub async fn play_doctor(&self, seed: &str) -> Result<bool> {
        let encoded = urlencoding::encode(seed);
        let params = format!("Seed={}&Radio=0&Action=1", encoded);
        self.send_command("Playback/PlayDoctor", &params).await?;

        // Poll for playback to start
        for _ in 0..10 {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let xml = self.send_command("Playback/Info", "").await?;
            if let Ok((_, total)) = Self::parse_playback_position(&xml) {
                if total > 0 {
                    info!("🎵 PlayDoctor started with {} tracks", total);
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Plays exactly the tracks of an album in order (P1.1)
    pub async fn play_precise_album(&self, album_name: &str) -> Result<()> {
        info!("💿 Precise play for album: {}", album_name);
        let tracks = self.get_album_tracks(album_name).await?;
        if !tracks.is_empty() {
            let keys: Vec<String> = tracks.into_iter().map(|t| t.1).collect();
            self.play_files(keys).await?;
        }
        Ok(())
    }

    /// Play a list of database keys (or file paths) sequentially
    pub async fn play_files(&self, keys: Vec<String>) -> Result<()> {
        if keys.is_empty() {
            return Ok(());
        }
        let keys_str = keys.join(",");
        let params = format!("Key={}", keys_str);
        self.send_command("Playback/PlayByKey", &params).await?;

        // Brief delay and status update
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        if let Ok(track) = self.what_is_playing().await {
            info!("🎵 Now playing: {}", track);
        }
        Ok(())
    }

    /// Returns a list of tracks for the given album (P2)
    pub async fn get_album_tracks(
        &self,
        album_name: &str,
    ) -> Result<Vec<(String, String, usize, usize)>> {
        let encoded = urlencoding::encode(album_name);
        // Include Disc # for multi-disc parity
        let params = format!("Query=[Album]=[{}]&Fields=Key,Name,Track #,Disc #", encoded);
        let xml = self.send_command("Files/Search", &params).await?;

        let mut tracks = Vec::new();
        let mut reader = Reader::from_str(&xml);
        let mut name = String::new();
        let mut key = String::new();
        let mut track_num = 0usize;
        let mut disc_num = 1usize;
        let mut in_field = false;
        let mut field_name = String::new();

        loop {
            match reader.read_event() {
                Ok(Event::Start(ref e)) if e.name().as_ref() == b"Field" => {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"Name" {
                            field_name = String::from_utf8_lossy(attr.value.as_ref()).to_string();
                            in_field = true;
                        }
                    }
                }
                Ok(Event::Text(ref e)) if in_field => {
                    if let Ok(text) = e.unescape() {
                        let val = text.to_string();
                        match field_name.as_str() {
                            "Name" => name = val,
                            "Key" => key = val,
                            "Track #" => track_num = val.parse().unwrap_or(0),
                            "Disc #" => disc_num = val.parse().unwrap_or(1),
                            _ => {}
                        }
                    }
                }
                Ok(Event::End(ref e)) if e.name().as_ref() == b"Field" => {
                    in_field = false;
                }
                Ok(Event::End(ref e)) if e.name().as_ref() == b"Item" => {
                    if !name.is_empty() {
                        tracks.push((name.clone(), key.clone(), track_num, disc_num));
                    }
                    name.clear();
                    key.clear();
                    track_num = 0;
                    disc_num = 1;
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => (),
            }
        }

        // Sort by Disc # then Track # (Python Parity Fix)
        tracks.sort_by(|a, b| match a.3.cmp(&b.3) {
            std::cmp::Ordering::Equal => a.2.cmp(&b.2),
            other => other,
        });
        Ok(tracks)
    }

    /// Returns a list of tracks for the given playlist (P2)
    pub async fn get_playlist_tracks(&self, playlist_name: &str) -> Result<Vec<(String, String)>> {
        let playlists = self.get_playlists().await;
        if let Some((_, id)) = playlists
            .iter()
            .find(|(n, _)| n.to_lowercase() == playlist_name.to_lowercase())
        {
            let params = format!("Playlist={}&PlaylistType=ID", id);
            let xml = self.send_command("Playlist/Files", &params).await?;

            let mut tracks = Vec::new();
            let mut reader = Reader::from_str(&xml);
            let mut name = String::new();
            let mut key = String::new();
            let mut in_field = false;
            let mut field_name = String::new();

            loop {
                match reader.read_event() {
                    Ok(Event::Start(ref e)) if e.name().as_ref() == b"Field" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"Name" {
                                field_name =
                                    String::from_utf8_lossy(attr.value.as_ref()).to_string();
                                in_field = true;
                            }
                        }
                    }
                    Ok(Event::Text(ref e)) if in_field => {
                        if let Ok(text) = e.unescape() {
                            let val = text.to_string();
                            match field_name.as_str() {
                                "Name" => name = val,
                                "Key" => key = val,
                                _ => {}
                            }
                        }
                    }
                    Ok(Event::End(ref e)) if e.name().as_ref() == b"Field" => {
                        in_field = false;
                    }
                    Ok(Event::End(ref e)) if e.name().as_ref() == b"Item" => {
                        if !name.is_empty() {
                            tracks.push((name.clone(), key.clone()));
                        }
                        name.clear();
                        key.clear();
                    }
                    Ok(Event::Eof) => break,
                    Err(_) => break,
                    _ => (),
                }
            }
            return Ok(tracks);
        }
        anyhow::bail!("Playlist not found: {}", playlist_name)
    }

    /// Attempts to launch JRiver Media Center 35 (Python Parity)
    pub async fn launch_jriver(&self) -> Result<bool> {
        info!("🚀 Launching JRiver Media Center 35...");

        let status = tokio::process::Command::new("mediacenter35")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();

        match status {
            Ok(_) => {
                let url = format!("{}Alive", self.url);
                let max_wait = 20; // 20 seconds polling
                info!(
                    "⏳ Waiting for JRiver to become ready (max {}s)...",
                    max_wait
                );

                for i in 1..=max_wait {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    if self.client.get(&url).send().await.is_ok() {
                        info!("✅ JRiver is ready after {}s", i);
                        return Ok(true);
                    }
                    debug!("... polling JRiver status ({}s)", i);
                }
                warn!(
                    "❌ JRiver launched but failed to respond after {}s",
                    max_wait
                );
                Ok(false)
            }
            Err(e) => {
                warn!("❌ Failed to launch 'mediacenter35': {}", e);
                Ok(false)
            }
        }
    }
}

#[async_trait]
impl MediaPlayer for JRiverPlayer {
    async fn play_genre(&self, genre: &str) -> Result<()> {
        // Use fuzzy matching to find best genre
        let matches = self.find_field_matches(genre, "Genre", 1).await;

        if let Some(best) = matches.first() {
            if best.score >= 0.6 {
                info!(
                    "🎯 Matched Genre: '{}' (Score: {:.2})",
                    best.value, best.score
                );
                let encoded = urlencoding::encode(&best.value);
                let params = format!("Seed=[Genre]=[{}]&Action=1", encoded);
                self.send_command("Playback/PlayDoctor", &params).await?;
                return Ok(());
            }
        }

        // Fallback to exact match
        info!("Playing genre: {}", genre);
        let encoded = urlencoding::encode(genre);
        let params = format!("Seed=[Genre]=[{}]&Action=1", encoded);
        self.send_command("Playback/PlayDoctor", &params).await?;
        Ok(())
    }

    async fn play_random(&self) -> Result<()> {
        info!("Playing random music");
        self.send_command("Playback/PlayDoctor", "Action=1").await?;
        Ok(())
    }

    async fn play_artist(&self, artist: &str) -> Result<()> {
        // Use fuzzy matching
        let matches = self.find_field_matches(artist, "Artist", 1).await;

        let target = if let Some(best) = matches.first() {
            if best.score >= 0.6 {
                info!(
                    "🎯 Matched Artist: '{}' (Score: {:.2})",
                    best.value, best.score
                );
                best.value.clone()
            } else {
                artist.to_string()
            }
        } else {
            artist.to_string()
        };

        info!("Playing artist: {}", target);
        let encoded = urlencoding::encode(&target);
        // Fix: Use Playback/Play for exact artist playback. Action=0 is "Replace and Play".
        let params = format!("Query=[Artist]=[{}]&Action=0&Zone=-1", encoded);
        self.send_command("Playback/Play", &params).await?;
        Ok(())
    }

    async fn play_album(&self, album: &str) -> Result<()> {
        // Use fuzzy matching
        let matches = self.find_field_matches(album, "Album", 1).await;

        let target = if let Some(best) = matches.first() {
            if best.score >= 0.6 {
                info!(
                    "🎯 Matched Album: '{}' (Score: {:.2})",
                    best.value, best.score
                );
                best.value.clone()
            } else {
                album.to_string()
            }
        } else {
            album.to_string()
        };

        info!("Playing album: {}", target);
        // Fix: Use Playback/Play for exact album playback. Action=0 is "Replace and Play".
        let encoded = urlencoding::encode(&target);
        // Play the exact album tracks, in order.
        let params = format!("Query=[Album]=[{}]&Action=0&Zone=-1", encoded);
        self.send_command("Playback/Play", &params).await?;
        Ok(())
    }

    async fn play_song(&self, song: &str) -> Result<()> {
        info!("Playing song: {}", song);
        let encoded = urlencoding::encode(song);
        // Fix: Use Playback/Play for exact song playback. Action=0 is "Replace and Play".
        let params = format!("Query=[Name]=[{}]&Action=0&Zone=-1", encoded);
        // Play the specific song(s)
        self.send_command("Playback/Play", &params).await?;
        Ok(())
    }

    async fn play_playlist(&self, playlist: &str, shuffle: bool) -> Result<()> {
        info!("Playing playlist: {}", playlist);

        let playlists = self.get_playlists().await;
        let names: Vec<String> = playlists.iter().map(|(n, _)| n.clone()).collect();

        let matches = find_matches(playlist, &names, 1, 0.6);

        if let Some(best) = matches.first() {
            if let Some((_, id)) = playlists.iter().find(|(n, _)| n == &best.value) {
                info!(
                    "🎯 Matched Playlist: '{}' (Score: {:.2})",
                    best.value, best.score
                );
                // Action=0 is Replace for PlayPlaylist
                let params = format!("Playlist={}&PlaylistType=ID&Action=0&Zone=-1", id);
                self.send_command("Playback/PlayPlaylist", &params).await?;

                if shuffle {
                    self.send_command("Playback/Shuffle", "Mode=reshuffle")
                        .await?;
                }
                return Ok(());
            }
        }

        warn!("Playlist not found: {}", playlist);
        Ok(())
    }

    async fn play_any(&self, query: &str) -> Result<Vec<SearchResult>> {
        info!("🔎 [JRiver] play_any searching for: '{}'", query);

        // Normalize query for classical music (P0.2)
        let normalized = Self::normalize_text(query);
        info!("🔎 [JRiver] Normalized query: '{}'", normalized);

        let mut candidates: Vec<SearchResult> = Vec::new();

        // Search Artists with contextual album expansion (P0.1)
        for m in self.find_field_matches(&normalized, "Artist", 5).await {
            candidates.push(SearchResult {
                display: format!("Artist: {}", m.value),
                value: m.value.clone(),
                result_type: SearchResultType::Artist,
                score: m.score,
            });

            // Contextual Album Expansion (Python Parity)
            if m.score > 0.8 {
                let albums = self.get_artist_albums(&m.value).await;
                for alb in albums {
                    // Avoid duplicates
                    if !candidates
                        .iter()
                        .any(|c| c.value == alb && c.result_type == SearchResultType::Album)
                    {
                        candidates.push(SearchResult {
                            display: format!("Album: {} (by {})", alb, m.value),
                            value: alb,
                            result_type: SearchResultType::Album,
                            score: m.score * 0.98,
                        });
                    }
                }
            }
        }

        // Search Songs (Name)
        for m in self.find_field_matches(&normalized, "Name", 5).await {
            candidates.push(SearchResult {
                display: format!("Song: {}", m.value),
                value: m.value,
                result_type: SearchResultType::Song,
                score: m.score,
            });
        }

        // Search Composers with contextual album expansion (P0.1)
        for m in self.find_field_matches(&normalized, "Composer", 5).await {
            candidates.push(SearchResult {
                display: format!("Composer: {}", m.value),
                value: m.value.clone(),
                result_type: SearchResultType::Artist, // Treat as artist-like
                score: m.score,
            });

            // Contextual Album Expansion for Composers (Python Parity)
            if m.score > 0.8 {
                let albums = self.get_artist_albums(&m.value).await;
                for alb in albums {
                    if !candidates
                        .iter()
                        .any(|c| c.value == alb && c.result_type == SearchResultType::Album)
                    {
                        candidates.push(SearchResult {
                            display: format!("Album: {} (by {})", alb, m.value),
                            value: alb,
                            result_type: SearchResultType::Album,
                            score: m.score * 0.98,
                        });
                    }
                }
            }
        }

        // Search Albums
        for m in self.find_field_matches(&normalized, "Album", 5).await {
            candidates.push(SearchResult {
                display: format!("Album: {}", m.value),
                value: m.value,
                result_type: SearchResultType::Album,
                score: m.score,
            });
        }

        // Search Playlists
        let playlists = self.get_playlists().await;
        let playlist_names: Vec<String> = playlists.iter().map(|(n, _)| n.clone()).collect();
        for m in find_matches(query, &playlist_names, 5, 0.6) {
            candidates.push(SearchResult {
                display: format!("Playlist: {}", m.value),
                value: m.value,
                result_type: SearchResultType::Playlist,
                score: m.score,
            });
        }

        info!(
            "🔎 [JRiver] Found {} candidates before threshold filtering",
            candidates.len()
        );

        // Sort by score
        candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Filter and limit
        candidates.retain(|c| c.score >= 0.6);
        candidates.truncate(25);

        info!(
            "🔎 [JRiver] Found {} valid candidates after filtering (cutoff 0.6)",
            candidates.len()
        );

        if candidates.is_empty() {
            info!(
                "⚠️ No fuzzy matches found, falling back to PlayDoctor for: '{}'",
                normalized
            );
            if let Err(e) = self.play_doctor(&normalized).await {
                warn!("PlayDoctor fallback failed: {}", e);
                return Ok(Vec::new());
            }
            // Return a special result to indicate PlayDoctor is active
            return Ok(vec![SearchResult {
                display: format!("PlayDoctor: {}", normalized),
                value: normalized,
                result_type: SearchResultType::Song, // Use Song as generic
                score: 1.0,
            }]);
        }

        Ok(candidates)
    }

    async fn get_all_artists(&self, limit: usize) -> Vec<String> {
        let artists = self.get_all_values("Artist").await;
        artists.into_iter().take(limit).collect()
    }

    async fn play_pause(&self) -> Result<()> {
        self.send_command("Playback/Pause", "").await?;
        Ok(())
    }

    async fn next_track(&self) -> Result<()> {
        self.send_command("Playback/Next", "").await?;
        // Brief delay for track change, then log what's playing (P1.2)
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        if let Ok(track) = self.what_is_playing().await {
            info!("🎵 Now playing: {}", track);
        }
        Ok(())
    }

    async fn previous_track(&self) -> Result<()> {
        self.send_command("Playback/Previous", "").await?;
        // Brief delay for track change, then log what's playing (P1.2)
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        if let Ok(track) = self.what_is_playing().await {
            info!("🎵 Now playing: {}", track);
        }
        Ok(())
    }

    async fn volume_up(&self) -> Result<()> {
        self.send_command("Playback/Volume", "Level=600").await?;
        Ok(())
    }

    async fn volume_down(&self) -> Result<()> {
        self.send_command("Playback/Volume", "Level=400").await?;
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        self.send_command("Playback/Stop", "").await?;
        Ok(())
    }

    async fn what_is_playing(&self) -> Result<String> {
        let xml = self.send_command("Playback/Info", "").await?;
        let mut reader = Reader::from_str(&xml);
        let mut name = String::new();
        let mut artist = String::new();
        let mut current_item = String::new();

        loop {
            match reader.read_event() {
                Ok(Event::Start(ref e)) if e.name().as_ref() == b"Item" => {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"Name" {
                            current_item = String::from_utf8_lossy(attr.value.as_ref()).to_string();
                        }
                    }
                }
                Ok(Event::Text(ref e)) => {
                    let val = e.unescape()?.to_string();
                    if current_item == "Name" {
                        name = val;
                    } else if current_item == "Artist" {
                        artist = val;
                    }
                }
                Ok(Event::End(ref e)) if e.name().as_ref() == b"Item" => {
                    current_item.clear();
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(e.into()),
                _ => (),
            }
        }

        if name.is_empty() {
            Ok("Nothing playing".to_string())
        } else {
            Ok(format!("{} by {}", name, artist))
        }
    }

    async fn get_artist_albums(&self, artist: &str) -> Vec<String> {
        use regex::Regex;
        use std::collections::HashSet;

        let tokens: Vec<&str> = artist.split_whitespace().collect();
        if tokens.is_empty() {
            return vec![];
        }

        // Search MCWS for files containing artist name
        let encoded = urlencoding::encode(artist);
        let params = format!("Query={}&Fields=Album,Artist,Composer,Name", encoded);

        let xml = match self.send_command("Files/Search", &params).await {
            Ok(xml) => xml,
            Err(_) => return vec![],
        };

        let mut albums: HashSet<String> = HashSet::new();
        let mut reader = Reader::from_str(&xml);
        let mut current_album = String::new();
        let mut full_text_parts: Vec<String> = Vec::new();
        let mut in_field = false;
        let mut field_name = String::new();

        loop {
            match reader.read_event() {
                Ok(Event::Start(ref e)) if e.name().as_ref() == b"Field" => {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"Name" {
                            field_name = String::from_utf8_lossy(attr.value.as_ref()).to_string();
                            in_field = true;
                        }
                    }
                }
                Ok(Event::Text(ref e)) if in_field => {
                    if let Ok(text) = e.unescape() {
                        let val = text.to_string();
                        if field_name == "Album" {
                            current_album = val.clone();
                        }
                        if matches!(
                            field_name.as_str(),
                            "Album" | "Artist" | "Composer" | "Name"
                        ) {
                            full_text_parts.push(val);
                        }
                    }
                }
                Ok(Event::End(ref e)) if e.name().as_ref() == b"Field" => {
                    in_field = false;
                }
                Ok(Event::End(ref e)) if e.name().as_ref() == b"Item" => {
                    if !current_album.is_empty() {
                        // Check if ALL tokens are present in combined fields
                        let full_text = full_text_parts.join(" ").to_lowercase();
                        let all_tokens_found = tokens.iter().all(|token| {
                            let pattern = format!(r"\b{}\b", regex::escape(&token.to_lowercase()));
                            Regex::new(&pattern)
                                .map(|re| re.is_match(&full_text))
                                .unwrap_or(false)
                        });

                        if all_tokens_found {
                            albums.insert(current_album.clone());
                        }
                    }
                    current_album.clear();
                    full_text_parts.clear();
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => (),
            }
        }

        let mut result: Vec<String> = albums.into_iter().collect();
        result.sort();
        result
    }

    async fn health_check(&self) -> bool {
        let url = format!("{}Alive", self.url);
        if self.client.get(&url).send().await.is_ok() {
            return true;
        }

        info!("⚠️ JRiver connection failed. Attempting auto-launch...");
        if let Ok(true) = self.launch_jriver().await {
            return true;
        }

        false
    }

    async fn list_tracks(&self) -> Vec<(String, String)> {
        // Implementation for listing tracks (Playing Now)
        // returns Vec<(title, key/path)>
        let mut tracks = Vec::new();
        if let Ok(xml) = self.send_command("Playback/Playlist", "").await {
            let mut reader = Reader::from_str(&xml);
            let mut current_name = String::new();
            let mut current_key = String::new();
            let mut in_field = false;
            let mut field_name = String::new();

            loop {
                match reader.read_event() {
                    Ok(Event::Start(ref e)) if e.name().as_ref() == b"Field" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"Name" {
                                field_name =
                                    String::from_utf8_lossy(attr.value.as_ref()).to_string();
                                in_field = true;
                            }
                        }
                    }
                    Ok(Event::Text(ref e)) if in_field => {
                        if let Ok(text) = e.unescape() {
                            if field_name == "Name" {
                                current_name = text.to_string();
                            } else if field_name == "Key" {
                                current_key = text.to_string();
                            }
                        }
                    }
                    Ok(Event::End(ref e)) if e.name().as_ref() == b"Field" => {
                        in_field = false;
                    }
                    Ok(Event::End(ref e)) if e.name().as_ref() == b"Item" => {
                        if !current_name.is_empty() {
                            tracks.push((current_name.clone(), current_key.clone()));
                        }
                        current_name.clear();
                        current_key.clear();
                    }
                    Ok(Event::Eof) => break,
                    Err(_) => break,
                    _ => (),
                }
            }
        }
        tracks
    }
}
