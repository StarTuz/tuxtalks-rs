//! Command processing module
//!
//! Handles voice command matching and action execution.

use crate::input::{parse_key, VirtualKeyboard};
use anyhow::Result;
use evdev::Key;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// A voice command binding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandBinding {
    /// Voice phrases that trigger this command
    pub triggers: Vec<String>,
    /// Key to press (e.g., "F1", "SPACE")
    pub key: String,
    /// Optional modifiers (e.g., ["CTRL", "SHIFT"])
    #[serde(default)]
    pub modifiers: Vec<String>,
}

/// Command processor that matches voice input to actions
pub struct CommandProcessor {
    bindings: HashMap<String, CommandBinding>,
    keyboard: Option<VirtualKeyboard>,
}

impl CommandProcessor {
    /// Create a new command processor
    pub fn new() -> Result<Self> {
        // Try to create virtual keyboard (requires uinput access)
        let keyboard = match VirtualKeyboard::new() {
            Ok(kb) => Some(kb),
            Err(e) => {
                warn!("⚠️ Could not create virtual keyboard: {}", e);
                warn!("   Run with: sudo ./tuxtalks or add user to 'input' group");
                None
            }
        };

        Ok(Self {
            bindings: HashMap::new(),
            keyboard,
        })
    }

    /// Add a command binding
    pub fn add_binding(&mut self, name: &str, binding: CommandBinding) {
        self.bindings.insert(name.to_string(), binding);
    }

    /// Add default demo bindings
    pub fn add_demo_bindings(&mut self) {
        let demo_bindings = vec![
            (
                "boost",
                CommandBinding {
                    triggers: vec!["boost".into(), "boost engines".into()],
                    key: "TAB".into(),
                    modifiers: vec![],
                },
            ),
            (
                "fire",
                CommandBinding {
                    triggers: vec!["fire".into(), "shoot".into()],
                    key: "SPACE".into(),
                    modifiers: vec![],
                },
            ),
            (
                "pause",
                CommandBinding {
                    triggers: vec!["pause".into(), "pause game".into()],
                    key: "ESC".into(),
                    modifiers: vec![],
                },
            ),
            (
                "screenshot",
                CommandBinding {
                    triggers: vec!["screenshot".into(), "take screenshot".into()],
                    key: "F12".into(),
                    modifiers: vec![],
                },
            ),
            (
                "save",
                CommandBinding {
                    triggers: vec!["quick save".into(), "save game".into()],
                    key: "S".into(),
                    modifiers: vec!["CTRL".into()],
                },
            ),
        ];

        for (name, binding) in demo_bindings {
            info!(
                "  {} -> {:?} + {}",
                binding.triggers.join(", "),
                binding.modifiers,
                binding.key
            );
            self.add_binding(name, binding);
        }
    }

    /// Process voice input and execute matching command
    pub fn process(&mut self, text: &str) -> Option<String> {
        let text_lower = text.to_lowercase();

        // Find matching command (collect data first to avoid borrow issues)
        let matched: Option<(String, CommandBinding)> =
            self.bindings.iter().find_map(|(name, binding)| {
                for trigger in &binding.triggers {
                    if text_lower.contains(trigger) {
                        info!("🎯 Matched command: {} (trigger: '{}')", name, trigger);
                        return Some((name.clone(), binding.clone()));
                    }
                }
                None
            });

        // Execute if matched
        if let Some((name, binding)) = matched {
            if let Err(e) = self.execute_binding(&binding) {
                warn!("❌ Failed to execute {}: {}", name, e);
            }
            return Some(name);
        }

        debug!("No command matched for: '{}'", text);
        None
    }

    /// Execute a command binding (press keys)
    fn execute_binding(&mut self, binding: &CommandBinding) -> Result<()> {
        let keyboard = self
            .keyboard
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("No virtual keyboard available"))?;

        let key = parse_key(&binding.key)
            .ok_or_else(|| anyhow::anyhow!("Unknown key: {}", binding.key))?;

        if binding.modifiers.is_empty() {
            // Simple key press
            keyboard.tap_key(key)?;
        } else {
            // Key combo with modifiers
            let modifiers: Vec<Key> = binding
                .modifiers
                .iter()
                .filter_map(|m| parse_key(m))
                .collect();
            keyboard.key_combo(&modifiers, key)?;
        }

        info!("⌨️ Pressed: {:?} + {:?}", binding.modifiers, binding.key);
        Ok(())
    }

    /// Check if we have a working keyboard
    pub fn has_keyboard(&self) -> bool {
        self.keyboard.is_some()
    }
}
