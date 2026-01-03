//! Game profile and bindings management
//!
//! Parsers for Elite Dangerous and X4 Foundations bindings files.

pub mod elite;
pub mod x4;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

use crate::commands::{Command, Macro, MacroStep};

/// A key binding from a game's config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBinding {
    pub action: String,
    pub primary_key: Option<String>,
    pub secondary_key: Option<String>,
    pub modifiers: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameType {
    EliteDangerous,
    X4Foundations,
    Generic,
}

/// A game profile with loaded bindings and commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameProfile {
    pub name: String,
    pub game_type: GameType,
    pub bindings_path: Option<PathBuf>,
    /// Raw bindings from the game (Tag -> Binding)
    pub raw_bindings: HashMap<String, KeyBinding>,
    /// Friendly Name -> Voice Triggers mappings
    pub voice_commands: HashMap<String, Vec<String>>,
    /// Macros defined for this profile
    pub macros: Vec<Macro>,
    /// Friendly Name -> Raw Tags mapping (e.g., "Lights" -> ["ShipSpotLightToggle", "Headlights"])
    pub virtual_tags: HashMap<String, Vec<String>>,
    /// Is this profile active?
    pub enabled: bool,
}

impl GameProfile {
    pub fn new(name: &str, game_type: GameType) -> Self {
        let mut profile = Self {
            name: name.to_string(),
            game_type,
            bindings_path: None,
            raw_bindings: HashMap::new(),
            voice_commands: HashMap::new(),
            macros: Vec::new(),
            virtual_tags: HashMap::new(),
            enabled: false,
        };

        // Initialize defaults based on game type
        match game_type {
            GameType::EliteDangerous => elite::init_defaults(&mut profile),
            GameType::X4Foundations => x4::init_defaults(&mut profile),
            _ => {}
        }

        profile
    }

    /// Load bindings from the game's config file
    pub fn load_bindings(&mut self) -> Result<usize> {
        let path = self
            .bindings_path
            .as_ref()
            .context("No bindings path set")?;

        let count = match self.game_type {
            GameType::EliteDangerous => elite::parse_bindings(path, &mut self.raw_bindings)?,
            GameType::X4Foundations => x4::parse_bindings(path, &mut self.raw_bindings)?,
            GameType::Generic => 0,
        };

        Ok(count)
    }

    /// Resolved Action Map: Friendly Name -> KeyBinding
    /// This resolves virtual tags to actual game bindings.
    pub fn resolve_actions(&self) -> HashMap<String, KeyBinding> {
        let mut action_map = HashMap::new();

        for (friendly_name, tags) in &self.virtual_tags {
            for tag in tags {
                if let Some(binding) = self.raw_bindings.get(tag) {
                    let mut resolved = binding.clone();
                    resolved.action = friendly_name.clone();
                    action_map.insert(friendly_name.clone(), resolved);
                    break; // Use first matching tag
                }
            }
        }

        action_map
    }

    /// Convert profile commands (Actions + Macros) into a format ready for the CommandProcessor
    pub fn get_processor_commands(&self) -> Vec<Command> {
        let mut commands = Vec::new();
        let action_map = self.resolve_actions();

        // Add actions
        for (friendly_name, triggers) in &self.voice_commands {
            if let Some(binding) = action_map.get(friendly_name) {
                if let Some(key) = &binding.primary_key {
                    commands.push(Command::Action {
                        name: friendly_name.clone(),
                        triggers: triggers.clone(),
                        key: key.clone(),
                        modifiers: binding.modifiers.clone(),
                    });
                }
            }
        }

        // Add macros
        for macro_def in &self.macros {
            commands.push(Command::Macro(macro_def.clone()));
        }

        commands
    }
}

/// Central manager for game profiles
pub struct GameManager {
    pub profiles: Vec<GameProfile>,
    pub active_profile_index: Option<usize>,
    config_dir: PathBuf,
}

impl GameManager {
    pub fn new() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .context("Could not find config directory")?
            .join("tuxtalks-rs");

        fs::create_dir_all(&config_dir)?;

        let mut manager = Self {
            profiles: Vec::new(),
            active_profile_index: None,
            config_dir,
        };

        manager.load_profiles().ok();

        // If no profiles loaded, add defaults
        if manager.profiles.is_empty() {
            manager.add_profile(GameProfile::new(
                "Elite Dangerous",
                GameType::EliteDangerous,
            ));
            manager.add_profile(GameProfile::new("X4 Foundations", GameType::X4Foundations));
            manager.save_profiles()?;
        }

        Ok(manager)
    }

    pub fn add_profile(&mut self, profile: GameProfile) {
        self.profiles.push(profile);
    }

    pub fn load_profiles(&mut self) -> Result<()> {
        let path = self.config_dir.join("profiles.json");
        if path.exists() {
            let content = fs::read_to_string(path)?;
            self.profiles = serde_json::from_str(&content)?;
            info!("📖 Loaded {} profiles", self.profiles.len());
        }
        Ok(())
    }

    pub fn save_profiles(&self) -> Result<()> {
        let path = self.config_dir.join("profiles.json");
        let content = serde_json::to_string_pretty(&self.profiles)?;
        fs::write(path, content)?;
        debug!("💾 Saved profiles");
        Ok(())
    }

    pub fn get_active_profile(&self) -> Option<&GameProfile> {
        self.active_profile_index
            .and_then(|idx| self.profiles.get(idx))
    }
}
