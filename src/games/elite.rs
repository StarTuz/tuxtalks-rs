//! Elite Dangerous bindings parser
//!
//! Parses .binds XML files from Elite Dangerous.

use anyhow::{Context, Result};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tracing::{debug, info};

use super::{GameProfile, KeyBinding};
use crate::commands::{Macro, MacroStep};

/// Initialize default virtual tags and voice commands for Elite Dangerous
pub fn init_defaults(profile: &mut GameProfile) {
    // Virtual Tags: Friendly Name -> [XML Tags]
    let virtual_tags = vec![
        ("External Panel", vec!["FocusLeftPanel", "TargetPanel"]),
        ("Internal Panel", vec!["FocusRightPanel", "SystemPanel"]),
        ("Role Panel", vec!["FocusRadarPanel", "RolePanel"]),
        ("Comms Panel", vec!["FocusCommsPanel", "QuickCommsPanel"]),
        ("Galaxy Map", vec!["GalaxyMapOpen"]),
        ("System Map", vec!["SystemMapOpen"]),
        ("Landing Gear", vec!["LandingGearToggle", "LandingGear"]),
        ("Cargo Scoop", vec!["ToggleCargoScoop", "CargoScoop"]),
        ("Flight Assist", vec!["ToggleFlightAssist", "FlightAssist"]),
        ("Boost", vec!["UseBoostJuice", "Boost"]),
        (
            "Frame Shift Drive",
            vec!["HyperSuperCombination", "Supercruise", "Hyperspace"],
        ),
        ("Lights", vec!["ShipSpotLightToggle", "Headlights"]),
        (
            "Hardpoints",
            vec!["DeployHardpointToggle", "DeployHardpoints"],
        ),
    ];

    for (friendly, tags) in virtual_tags {
        profile.virtual_tags.insert(
            friendly.to_string(),
            tags.iter().map(|s| s.to_string()).collect(),
        );
    }

    // Voice Commands: Friendly Name -> [Triggers]
    let voice_commands = vec![
        ("Boost", vec!["boost", "boost engines", "afterburner"]),
        ("Landing Gear", vec!["landing gear", "gear", "deploy gear"]),
        ("Cargo Scoop", vec!["cargo scoop", "scoop", "utility scoop"]),
        ("Lights", vec!["lights", "ship lights", "headlights"]),
        ("Galaxy Map", vec!["galaxy map", "open map", "star map"]),
        (
            "Hardpoints",
            vec!["hard points", "weapons", "deploy weapons"],
        ),
        (
            "Frame Shift Drive",
            vec!["engage", "warp", "jump", "hyperspace"],
        ),
    ];

    for (friendly, triggers) in voice_commands {
        profile.voice_commands.insert(
            friendly.to_string(),
            triggers.iter().map(|s| s.to_string()).collect(),
        );
    }

    // Process names for auto-detection
    profile.process_names = vec![
        "EliteDangerous64.exe".into(),
        "EliteDangerous.exe".into(),
        "EDLaunch.exe".into(),
    ];

    // Path discriminators for Proton/GOG parity
    profile.path_discriminators = vec!["steamapps".into(), "compatdata".into(), "gog".into()];

    // Default Macros
    profile.macros.push(Macro {
        name: "RequestDocking".into(),
        triggers: vec!["request docking".into(), "docking request".into()],
        steps: vec![
            MacroStep {
                action: "External Panel".into(),
                delay: 500,
                ..Default::default()
            },
            MacroStep {
                action: "CycleNextPanel".into(),
                delay: 200,
                ..Default::default()
            },
            MacroStep {
                action: "CycleNextPanel".into(),
                delay: 200,
                ..Default::default()
            },
            MacroStep {
                action: "UI_Select".into(),
                delay: 200,
                ..Default::default()
            },
            MacroStep {
                action: "UI_Down".into(),
                delay: 200,
                ..Default::default()
            },
            MacroStep {
                action: "UI_Select".into(),
                delay: 200,
                ..Default::default()
            },
            MacroStep {
                action: "External Panel".into(),
                delay: 200,
                ..Default::default()
            },
        ],
    });
}

/// Parse Elite Dangerous .binds XML file
pub fn parse_bindings(path: &Path, bindings: &mut HashMap<String, KeyBinding>) -> Result<usize> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read bindings file: {}", path.display()))?;

    let mut reader = Reader::from_str(&content);
    reader.config_mut().trim_text(true);

    let mut current_action: Option<String> = None;
    let mut current_binding = KeyBinding {
        action: String::new(),
        primary_key: None,
        secondary_key: None,
        modifiers: vec![],
    };
    let mut count = 0;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                // Action elements (e.g., <FirePrimaryWeapon>, <Boost>)
                if !is_meta_tag(&tag_name) && current_action.is_none() {
                    current_action = Some(tag_name.clone());
                    current_binding = KeyBinding {
                        action: tag_name.clone(),
                        primary_key: None,
                        secondary_key: None,
                        modifiers: vec![],
                    };
                }

                // Primary/Secondary binding elements
                if let Some(ref _action) = current_action {
                    if tag_name == "Primary" || tag_name == "Secondary" {
                        // Extract Device and Key attributes
                        let mut device = String::new();
                        let mut key = String::new();

                        for attr in e.attributes().flatten() {
                            let attr_name = String::from_utf8_lossy(attr.key.as_ref());
                            let attr_value = String::from_utf8_lossy(&attr.value);

                            match attr_name.as_ref() {
                                "Device" => device = attr_value.to_string(),
                                "Key" => key = attr_value.to_string(),
                                _ => {}
                            }
                        }

                        // Only capture keyboard bindings
                        if device == "Keyboard" && !key.is_empty() {
                            let normalized_key = normalize_ed_key(&key);
                            if tag_name == "Primary" {
                                current_binding.primary_key = Some(normalized_key);
                            } else {
                                current_binding.secondary_key = Some(normalized_key);
                            }
                        }
                    }

                    // Modifier elements
                    if tag_name == "Modifier" {
                        for attr in e.attributes().flatten() {
                            let attr_name = String::from_utf8_lossy(attr.key.as_ref());
                            if attr_name == "Key" {
                                let key = String::from_utf8_lossy(&attr.value);
                                current_binding.modifiers.push(normalize_ed_key(&key));
                            }
                        }
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                // End of action element
                if Some(&tag_name) == current_action.as_ref() {
                    if current_binding.primary_key.is_some()
                        || current_binding.secondary_key.is_some()
                    {
                        debug!(
                            "ED Binding: {} -> {:?} (mods: {:?})",
                            current_binding.action,
                            current_binding.primary_key,
                            current_binding.modifiers
                        );
                        bindings.insert(current_binding.action.clone(), current_binding.clone());
                        count += 1;
                    }
                    current_action = None;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(anyhow::anyhow!("XML parse error: {}", e));
            }
            _ => {}
        }
    }

    info!(
        "📂 Parsed {} Elite Dangerous bindings from {}",
        count,
        path.display()
    );
    Ok(count)
}

/// Check if tag is a meta/container tag (not an action)
fn is_meta_tag(tag: &str) -> bool {
    matches!(
        tag,
        "Root" | "KeyboardLayout" | "Primary" | "Secondary" | "Modifier" | "Binding"
    )
}

/// Normalize Elite Dangerous key names to standard format
fn normalize_ed_key(key: &str) -> String {
    // ED uses format like "Key_A", "Key_Space", "Key_F1"
    let key = key.strip_prefix("Key_").unwrap_or(key);

    match key.to_uppercase().as_str() {
        "SPACE" => "SPACE".to_string(),
        "ESCAPE" => "ESC".to_string(),
        "RETURN" => "ENTER".to_string(),
        "BACKSPACE" => "BACKSPACE".to_string(),
        "TAB" => "TAB".to_string(),
        "DELETE" => "DELETE".to_string(),
        "INSERT" => "INSERT".to_string(),
        "HOME" => "HOME".to_string(),
        "END" => "END".to_string(),
        "PAGEUP" | "PRIOR" => "PAGEUP".to_string(),
        "PAGEDOWN" | "NEXT" => "PAGEDOWN".to_string(),
        "UPARROW" | "UP" => "UP".to_string(),
        "DOWNARROW" | "DOWN" => "DOWN".to_string(),
        "LEFTARROW" | "LEFT" => "LEFT".to_string(),
        "RIGHTARROW" | "RIGHT" => "RIGHT".to_string(),
        "LEFTSHIFT" => "LSHIFT".to_string(),
        "RIGHTSHIFT" => "RSHIFT".to_string(),
        "LEFTCONTROL" => "LCTRL".to_string(),
        "RIGHTCONTROL" => "RCTRL".to_string(),
        "LEFTALT" => "LALT".to_string(),
        "RIGHTALT" => "RALT".to_string(),
        other => other.to_uppercase(),
    }
}
