//! Player Manager
//!
//! Manages runtime player switching for TuxTalks.
//! Allows switching between different media players (JRiver, Strawberry, MPRIS)
//! via voice commands.

use crate::config::Config;
use crate::library::LocalLibrary;
use crate::players::{get_player, MediaPlayer};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Available player types
#[derive(Debug, Clone, PartialEq)]
pub enum PlayerType {
    JRiver,
    Strawberry,
    Elisa,
    Mpris,
}

impl std::str::FromStr for PlayerType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "jriver" | "j river" | "jriver media center" => Ok(PlayerType::JRiver),
            "strawberry" | "strawberry music" => Ok(PlayerType::Strawberry),
            "elisa" => Ok(PlayerType::Elisa),
            "mpris" | "generic" => Ok(PlayerType::Mpris),
            _ => Err(()),
        }
    }
}

impl PlayerType {
    /// Get player ID string for config
    pub fn id(&self) -> &'static str {
        match self {
            PlayerType::JRiver => "jriver",
            PlayerType::Strawberry => "strawberry",
            PlayerType::Elisa => "elisa",
            PlayerType::Mpris => "mpris",
        }
    }

    /// Get display name
    pub fn display_name(&self) -> &'static str {
        match self {
            PlayerType::JRiver => "JRiver Media Center",
            PlayerType::Strawberry => "Strawberry Music Player",
            PlayerType::Elisa => "Elisa Music Player",
            PlayerType::Mpris => "MPRIS (Generic)",
        }
    }

    /// Parse from string (Legacy/Convenience)
    pub fn parse(s: &str) -> Option<Self> {
        s.parse().ok()
    }

    /// Get all available player types
    pub fn all() -> Vec<PlayerType> {
        vec![
            PlayerType::JRiver,
            PlayerType::Strawberry,
            PlayerType::Elisa,
            PlayerType::Mpris,
        ]
    }
}

/// Manages the active media player and allows runtime switching
pub struct PlayerManager {
    /// Current player
    current_player: Arc<RwLock<Box<dyn MediaPlayer>>>,
    /// Current player type
    current_type: RwLock<PlayerType>,
    /// Local library (needed for player creation)
    library: Arc<LocalLibrary>,
    /// Config (for player-specific settings)
    config: Config,
}

impl PlayerManager {
    /// Create a new PlayerManager with the configured default player
    pub fn new(config: Config, library: Arc<LocalLibrary>) -> Self {
        let player = get_player(&config, library.clone());
        let player_type = PlayerType::parse(&config.player).unwrap_or(PlayerType::Mpris);

        info!(
            "🎵 PlayerManager initialized with: {}",
            player_type.display_name()
        );

        Self {
            current_player: Arc::new(RwLock::new(player)),
            current_type: RwLock::new(player_type),
            library,
            config,
        }
    }

    /// Get the current player
    pub fn player(&self) -> Arc<RwLock<Box<dyn MediaPlayer>>> {
        self.current_player.clone()
    }

    /// Get the local library
    pub fn library(&self) -> Arc<LocalLibrary> {
        self.library.clone()
    }

    /// Get current player type
    pub async fn current_type(&self) -> PlayerType {
        self.current_type.read().await.clone()
    }

    /// Switch to a different player
    pub async fn switch_to(&self, player_type: PlayerType) -> Result<String, String> {
        let current = self.current_type().await;

        if current == player_type {
            return Ok(format!("Already using {}", player_type.display_name()));
        }

        // Create new config with updated player
        let mut new_config = self.config.clone();
        new_config.player = player_type.id().to_string();

        // Create new player
        let new_player = get_player(&new_config, self.library.clone());

        // Update current player
        {
            let mut player_guard = self.current_player.write().await;
            *player_guard = new_player;
        }

        // Update current type
        {
            let mut type_guard = self.current_type.write().await;
            *type_guard = player_type.clone();
        }

        info!("🔄 Switched to player: {}", player_type.display_name());
        Ok(format!("Switched to {}", player_type.display_name()))
    }

    /// Switch to a player by name (voice command handler)
    pub async fn switch_by_name(&self, name: &str) -> Result<String, String> {
        if let Some(player_type) = PlayerType::parse(name) {
            self.switch_to(player_type).await
        } else {
            Err(format!(
                "Unknown player: {}. Available: jriver, strawberry, elisa, mpris",
                name
            ))
        }
    }

    /// Get list of available players for selection
    pub async fn available_players(&self) -> Vec<(String, String, bool)> {
        let current = self.current_type().await;

        PlayerType::all()
            .into_iter()
            .map(|p| {
                let is_current = p == current;
                let name = if is_current {
                    format!("{} (current)", p.display_name())
                } else {
                    p.display_name().to_string()
                };
                (name, p.id().to_string(), is_current)
            })
            .collect()
    }

    /// Parse player switch command from text
    ///
    /// Matches:
    /// - "switch to strawberry"
    /// - "change player to jriver"
    /// - "use elisa"
    pub fn parse_switch_command(text: &str) -> Option<String> {
        let text = text.to_lowercase();

        // Direct patterns
        let patterns = [
            "switch to ",
            "switch player to ",
            "change to ",
            "change player to ",
            "use ",
        ];

        for pattern in patterns {
            if text.starts_with(pattern) {
                let player_name = text.replace(pattern, "").trim().to_string();
                if !player_name.is_empty() {
                    return Some(player_name);
                }
            }
        }

        // Check for "player" command to show selection
        if text == "player"
            || text == "players"
            || text == "change player"
            || text == "switch player"
        {
            return Some("__show_list__".to_string());
        }

        None
    }
}
