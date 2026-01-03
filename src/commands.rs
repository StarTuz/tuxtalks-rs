//! Command processing module
//!
//! Handles voice command matching and action execution.

use crate::input::{parse_key, VirtualKeyboard};
use anyhow::Result;
use evdev::Key;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// A step in a macro
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroStep {
    /// Action ID to execute
    pub action: String,
    /// Delay in milliseconds after this step
    #[serde(default)]
    pub delay: u64,
}

/// A macro consisting of multiple steps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Macro {
    pub name: String,
    pub triggers: Vec<String>,
    pub steps: Vec<MacroStep>,
}

/// A voice command binding
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Command {
    /// Single key press action
    Action {
        name: String,
        triggers: Vec<String>,
        key: String,
        #[serde(default)]
        modifiers: Vec<String>,
    },
    /// Sequence of actions
    Macro(Macro),
}

/// Command processor that matches voice input to actions
pub struct CommandProcessor {
    commands: Vec<Command>,
    keyboard: Option<VirtualKeyboard>,
    /// Map of Action ID -> KeyBinding (populated by the active game profile)
    action_map: HashMap<String, crate::games::KeyBinding>,
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
            commands: Vec::new(),
            keyboard,
            action_map: HashMap::new(),
        })
    }

    /// Update the action map from the current game profile
    pub fn set_action_map(&mut self, map: HashMap<String, crate::games::KeyBinding>) {
        self.action_map = map;
    }

    /// Add a command
    pub fn add_command(&mut self, command: Command) {
        self.commands.push(command);
    }

    /// Add default demo bindings
    pub fn add_demo_bindings(&mut self) {
        self.add_command(Command::Action {
            name: "boost".into(),
            triggers: vec!["boost".into(), "boost engines".into()],
            key: "TAB".into(),
            modifiers: vec![],
        });
        self.add_command(Command::Action {
            name: "fire".into(),
            triggers: vec!["fire".into(), "shoot".into()],
            key: "SPACE".into(),
            modifiers: vec![],
        });
    }

    /// Process voice input and execute matching command
    pub fn process(&mut self, text: &str) -> Option<String> {
        let text_lower = text.to_lowercase();

        // Find matching command
        let matched = self
            .commands
            .iter()
            .find(|cmd| {
                let triggers = match cmd {
                    Command::Action { triggers, .. } => triggers,
                    Command::Macro(m) => &m.triggers,
                };
                triggers.iter().any(|t| text_lower.contains(t))
            })
            .cloned();

        // Execute if matched
        if let Some(cmd) = matched {
            let name = match &cmd {
                Command::Action { name, .. } => name.clone(),
                Command::Macro(m) => m.name.clone(),
            };

            info!("🎯 Matched command: {}", name);
            if let Err(e) = self.execute_command(cmd) {
                warn!("❌ Failed to execute {}: {}", name, e);
            }
            return Some(name);
        }

        debug!("No command matched for: '{}'", text);
        None
    }

    /// Execute a command
    fn execute_command(&mut self, command: Command) -> Result<()> {
        match command {
            Command::Action { key, modifiers, .. } => self.press_keys(&key, &modifiers),
            Command::Macro(m) => {
                info!("📜 Executing macro: {}", m.name);
                for step in m.steps {
                    let binding = self.action_map.get(&step.action).cloned();

                    if let Some(binding) = binding {
                        if let Some(key) = &binding.primary_key {
                            self.press_keys(key, &binding.modifiers)?;
                        }
                    } else {
                        warn!("⚠️ Unknown action in macro: {}", step.action);
                    }

                    if step.delay > 0 {
                        std::thread::sleep(std::time::Duration::from_millis(step.delay));
                    }
                }
                Ok(())
            }
        }
    }

    /// Helper to press keys
    fn press_keys(&mut self, key_str: &str, modifier_strs: &[String]) -> Result<()> {
        let keyboard = self
            .keyboard
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("No virtual keyboard available"))?;

        let key = parse_key(key_str).ok_or_else(|| anyhow::anyhow!("Unknown key: {}", key_str))?;

        if modifier_strs.is_empty() {
            keyboard.tap_key(key)?;
        } else {
            let modifiers: Vec<Key> = modifier_strs.iter().filter_map(|m| parse_key(m)).collect();
            keyboard.key_combo(&modifiers, key)?;
        }

        debug!("⌨️ Pressed: {:?} + {:?}", modifier_strs, key_str);
        Ok(())
    }

    /// Check if we have a working keyboard
    pub fn has_keyboard(&self) -> bool {
        self.keyboard.is_some()
    }
}
