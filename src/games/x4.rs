//! X4 Foundations bindings parser
//!
//! Parses inputmap.xml files from X4 Foundations.

use anyhow::{Context, Result};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tracing::{debug, info};

use super::{GameProfile, KeyBinding};

/// Initialize default virtual tags and voice commands for X4 Foundations
pub fn init_defaults(profile: &mut GameProfile) {
    let virtual_tags = vec![
        ("Boost", vec!["INPUT_ACTION_BOOST", "INPUT_STATE_BOOST"]),
        ("Scan Mode", vec!["INPUT_ACTION_TOGGLE_SCAN_MODE"]),
        ("Travel Mode", vec!["INPUT_ACTION_TOGGLE_TRAVEL_MODE"]),
        ("Landing Gear", vec!["INPUT_ACTION_TOGGLE_LANDING_GEAR"]),
        ("Map", vec!["INPUT_ACTION_OPEN_MAP"]),
        ("Interact", vec!["INPUT_ACTION_INTERACT"]),
    ];

    for (friendly, tags) in virtual_tags {
        profile.virtual_tags.insert(
            friendly.to_string(),
            tags.iter().map(|s| s.to_string()).collect(),
        );
    }

    let voice_commands = vec![
        ("Boost", vec!["boost".into(), "burn".into()]),
        ("Scan Mode", vec!["scan mode".into(), "toggle scan".into()]),
        (
            "Travel Mode",
            vec!["travel mode".into(), "engage travel".into()],
        ),
        ("Map", vec!["open map".into(), "show map".into()]),
        ("Landing Gear", vec!["landing gear".into(), "gear".into()]),
    ];

    for (friendly, triggers) in voice_commands {
        profile
            .voice_commands
            .insert(friendly.to_string(), triggers);
    }

    // Process names for auto-detection
    profile.process_names = vec!["X4.exe".into(), "X4".into()];
}

/// Parse X4 Foundations inputmap.xml file
pub fn parse_bindings(path: &Path, bindings: &mut HashMap<String, KeyBinding>) -> Result<usize> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read bindings file: {}", path.display()))?;

    let mut reader = Reader::from_str(&content);
    reader.config_mut().trim_text(true);

    let mut count = 0;

    loop {
        match reader.read_event() {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                if tag_name == "action" {
                    let mut action_id = String::new();
                    let mut input_code = String::new();

                    for attr in e.attributes().flatten() {
                        let attr_name = String::from_utf8_lossy(attr.key.as_ref());
                        let attr_value = String::from_utf8_lossy(&attr.value);

                        match attr_name.as_ref() {
                            "id" => action_id = attr_value.to_string(),
                            "input" => input_code = attr_value.to_string(),
                            _ => {}
                        }
                    }

                    if !action_id.is_empty() && !input_code.is_empty() {
                        if let Some(key) = parse_x4_input(&input_code) {
                            let binding = KeyBinding {
                                action: action_id.clone(),
                                primary_key: Some(key),
                                secondary_key: None,
                                modifiers: vec![],
                            };

                            debug!("X4 Binding: {} -> {:?}", action_id, binding.primary_key);
                            bindings.insert(action_id, binding);
                            count += 1;
                        }
                    }
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
        "📂 Parsed {} X4 Foundations bindings from {}",
        count,
        path.display()
    );
    Ok(count)
}

/// Parse X4 input code to key name
fn parse_x4_input(input: &str) -> Option<String> {
    // X4 uses codes like "INPUT_KEY_A", "INPUT_KEY_SPACE"
    let key = input.strip_prefix("INPUT_KEY_")?;

    Some(match key.to_uppercase().as_str() {
        "SPACE" => "SPACE".to_string(),
        "ESCAPE" => "ESC".to_string(),
        "RETURN" | "ENTER" => "ENTER".to_string(),
        "BACKSPACE" => "BACKSPACE".to_string(),
        "TAB" => "TAB".to_string(),
        "DELETE" => "DELETE".to_string(),
        "INSERT" => "INSERT".to_string(),
        "HOME" => "HOME".to_string(),
        "END" => "END".to_string(),
        "PAGEUP" | "PRIOR" => "PAGEUP".to_string(),
        "PAGEDOWN" | "NEXT" => "PAGEDOWN".to_string(),
        "UP" => "UP".to_string(),
        "DOWN" => "DOWN".to_string(),
        "LEFT" => "LEFT".to_string(),
        "RIGHT" => "RIGHT".to_string(),
        "LSHIFT" | "LEFTSHIFT" => "LSHIFT".to_string(),
        "RSHIFT" | "RIGHTSHIFT" => "RSHIFT".to_string(),
        "LCTRL" | "LEFTCONTROL" => "LCTRL".to_string(),
        "RCTRL" | "RIGHTCONTROL" => "RCTRL".to_string(),
        "LALT" | "LEFTALT" => "LALT".to_string(),
        "RALT" | "RIGHTALT" => "RALT".to_string(),
        other => other.to_uppercase(),
    })
}
