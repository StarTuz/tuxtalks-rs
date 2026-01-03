//! Game profile and bindings management
//!
//! Parsers for Elite Dangerous and X4 Foundations bindings files.

pub mod elite;
pub mod x4;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// A game profile with loaded bindings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameProfile {
    pub name: String,
    pub game_type: GameType,
    pub bindings_path: Option<PathBuf>,
    /// Map of action name -> key binding
    pub bindings: HashMap<String, KeyBinding>,
    /// Map of friendly name -> voice triggers
    pub voice_commands: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameType {
    EliteDangerous,
    X4Foundations,
    Generic,
}

/// A key binding from a game's config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBinding {
    pub action: String,
    pub primary_key: Option<String>,
    pub secondary_key: Option<String>,
    pub modifiers: Vec<String>,
}

impl GameProfile {
    /// Create an empty profile
    pub fn new(name: &str, game_type: GameType) -> Self {
        Self {
            name: name.to_string(),
            game_type,
            bindings_path: None,
            bindings: HashMap::new(),
            voice_commands: HashMap::new(),
        }
    }

    /// Load bindings from file based on game type
    pub fn load_bindings(&mut self, path: &PathBuf) -> Result<usize> {
        self.bindings_path = Some(path.clone());

        let count = match self.game_type {
            GameType::EliteDangerous => elite::parse_bindings(path, &mut self.bindings)?,
            GameType::X4Foundations => x4::parse_bindings(path, &mut self.bindings)?,
            GameType::Generic => 0,
        };

        Ok(count)
    }

    /// Get the key binding for an action
    pub fn get_binding(&self, action: &str) -> Option<&KeyBinding> {
        self.bindings.get(action)
    }

    /// Add a voice command mapping
    pub fn add_voice_command(&mut self, action: &str, triggers: Vec<String>) {
        self.voice_commands.insert(action.to_string(), triggers);
    }
}
