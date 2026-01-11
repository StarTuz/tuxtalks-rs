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

use crate::commands::{Macro, MacroStep};

/// Initialize default virtual tags and voice commands for X4 Foundations
pub fn init_defaults(profile: &mut GameProfile) {
    let virtual_tags_data = vec![
        (
            "Stop Engines",
            vec![
                "INPUT_ACTION_STOP",
                "INPUT_STATE_STOP_SHIP",
                "INPUT_STATE_DECCELERATE",
            ],
        ),
        ("Match Speed", vec!["INPUT_STATE_MATCH_SPEED"]),
        ("Boost", vec!["INPUT_ACTION_BOOST", "INPUT_STATE_BOOST"]),
        (
            "Travel Mode",
            vec!["INPUT_STATE_TRAVEL_MODE", "INPUT_ACTION_TOGGLE_TRAVEL_MODE"],
        ),
        (
            "Scan Mode",
            vec!["INPUT_STATE_SCAN_MODE", "INPUT_ACTION_TOGGLE_SCAN_MODE"],
        ),
        (
            "Long Range Scan",
            vec![
                "INPUT_STATE_LONG_RANGE_SCAN",
                "INPUT_ACTION_TOGGLE_LONGRANGE_SCAN_MODE",
            ],
        ),
        ("Fire Primary", vec!["INPUT_STATE_FIRE_PRIMARY"]),
        ("Fire Secondary", vec!["INPUT_STATE_FIRE_SECONDARY"]),
        ("Lower Shields", vec!["INPUT_STATE_LOWER_SHIELDS"]),
        (
            "Select Primary Weapon Group 1",
            vec!["INPUT_ACTION_SELECT_PRIMARY_WEAPONGROUP_1"],
        ),
        (
            "Select Primary Weapon Group 2",
            vec!["INPUT_ACTION_SELECT_PRIMARY_WEAPONGROUP_2"],
        ),
        (
            "Select Primary Weapon Group 3",
            vec!["INPUT_ACTION_SELECT_PRIMARY_WEAPONGROUP_3"],
        ),
        (
            "Select Primary Weapon Group 4",
            vec!["INPUT_ACTION_SELECT_PRIMARY_WEAPONGROUP_4"],
        ),
        (
            "Next Primary Weapon Group",
            vec!["INPUT_ACTION_CYCLE_NEXT_PRIMARY_WEAPONGROUP"],
        ),
        (
            "Previous Primary Weapon Group",
            vec!["INPUT_ACTION_CYCLE_PREV_PRIMARY_WEAPONGROUP"],
        ),
        (
            "Select Secondary Weapon Group 1",
            vec!["INPUT_ACTION_SELECT_SECONDARY_WEAPONGROUP_1"],
        ),
        (
            "Select Secondary Weapon Group 2",
            vec!["INPUT_ACTION_SELECT_SECONDARY_WEAPONGROUP_2"],
        ),
        (
            "Select Secondary Weapon Group 3",
            vec!["INPUT_ACTION_SELECT_SECONDARY_WEAPONGROUP_3"],
        ),
        (
            "Select Secondary Weapon Group 4",
            vec!["INPUT_ACTION_SELECT_SECONDARY_WEAPONGROUP_4"],
        ),
        (
            "Next Secondary Weapon Group",
            vec!["INPUT_ACTION_CYCLE_NEXT_SECONDARY_WEAPONGROUP"],
        ),
        (
            "Previous Secondary Weapon Group",
            vec!["INPUT_ACTION_CYCLE_PREV_SECONDARY_WEAPONGROUP"],
        ),
        ("Toggle Aim Assist", vec!["INPUT_ACTION_TOGGLE_AIM_ASSIST"]),
        ("Next Ammunition", vec!["INPUT_ACTION_NEXT_AMMUNITION"]),
        (
            "Deploy Countermeasures",
            vec!["INPUT_ACTION_DEPLOY_COUNTERMEASURES"],
        ),
        ("Map", vec!["INPUT_ACTION_OPEN_MAP"]),
        ("Undock", vec!["INPUT_ACTION_UNDOCK"]),
        ("Interact", vec!["INPUT_ACTION_INTERACT"]),
        ("Get Up", vec!["INPUT_ACTION_GET_UP"]),
        ("Next Target", vec!["INPUT_STATE_NEXT_TARGET"]),
        ("Previous Target", vec!["INPUT_STATE_PREV_TARGET"]),
        ("Next Subcomponent", vec!["INPUT_STATE_NEXT_SUBCOMPONENT"]),
        (
            "Previous Subcomponent",
            vec!["INPUT_STATE_PREV_SUBCOMPONENT"],
        ),
        (
            "Toggle Mouse Cursor",
            vec!["INPUT_STATE_TOGGLE_MOUSE_CURSOR"],
        ),
        ("Direct Mouse Mode", vec!["INPUT_STATE_MODE_DIRECT_MOUSE"]),
        ("Steering Mode", vec!["INPUT_STATE_MODE_STEERING"]),
        ("Request Docking", vec!["INPUT_ACTION_REQUEST_DOCK"]),
        ("Target View", vec!["INPUT_STATE_CAMERA_TARGET_VIEW"]),
        ("External View", vec!["INPUT_STATE_CAMERA_EXTERNAL_VIEW"]),
        ("Cockpit View", vec!["INPUT_STATE_CAMERA_COCKPIT_VIEW"]),
        ("Collect Loot", vec!["INPUT_ACTION_DRONE_COLLECT"]),
        ("Drones Attack", vec!["INPUT_ACTION_DRONE_ATTACK"]),
        ("Help", vec!["INPUT_ACTION_OPEN_HELP_MENU"]),
        ("Info", vec!["INPUT_ACTION_OPEN_INFO_MENU"]),
        ("Player Menu", vec!["INPUT_ACTION_OPEN_PLAYER_MENU"]),
        ("Mission Manager", vec!["INPUT_ACTION_OPEN_MISSHION_MENU"]),
    ];

    for (friendly, tags) in virtual_tags_data {
        profile.virtual_tags.insert(
            friendly.to_string(),
            tags.iter().map(|s| s.to_string()).collect(),
        );
    }

    let voice_commands_data = vec![
        (
            "Stop Engines",
            vec!["stop engines".into(), "stop".into(), "all stop".into()],
        ),
        ("Match Speed", vec!["match speed".into()]),
        ("Boost", vec!["boost".into(), "engine boost".into()]),
        (
            "Travel Mode",
            vec!["travel mode".into(), "engage travel mode".into()],
        ),
        ("Scan Mode", vec!["scan mode".into(), "scanner".into()]),
        (
            "Long Range Scan",
            vec!["long range scan".into(), "pulse scan".into()],
        ),
        ("Fire Primary", vec!["fire".into(), "fire weapons".into()]),
        (
            "Fire Secondary",
            vec!["fire missiles".into(), "fire secondary".into()],
        ),
        ("Lower Shields", vec!["drop shields".into()]),
        (
            "Select Primary Weapon Group 1",
            vec![
                "primary weapon group one".into(),
                "group one".into(),
                "group 1".into(),
                "select primary one".into(),
            ],
        ),
        (
            "Select Primary Weapon Group 2",
            vec![
                "primary weapon group two".into(),
                "group two".into(),
                "group 2".into(),
                "select primary two".into(),
            ],
        ),
        (
            "Select Primary Weapon Group 3",
            vec![
                "primary weapon group three".into(),
                "group three".into(),
                "group 3".into(),
                "select primary three".into(),
            ],
        ),
        (
            "Select Primary Weapon Group 4",
            vec![
                "primary weapon group four".into(),
                "group four".into(),
                "group 4".into(),
                "select primary four".into(),
            ],
        ),
        (
            "Next Primary Weapon Group",
            vec!["next primary group".into(), "next weapon group".into()],
        ),
        (
            "Previous Primary Weapon Group",
            vec![
                "previous primary group".into(),
                "previous weapon group".into(),
            ],
        ),
        (
            "Select Secondary Weapon Group 1",
            vec![
                "secondary weapon group one".into(),
                "select secondary one".into(),
            ],
        ),
        (
            "Select Secondary Weapon Group 2",
            vec![
                "secondary weapon group two".into(),
                "select secondary two".into(),
            ],
        ),
        (
            "Select Secondary Weapon Group 3",
            vec![
                "secondary weapon group three".into(),
                "select secondary three".into(),
            ],
        ),
        (
            "Select Secondary Weapon Group 4",
            vec![
                "secondary weapon group four".into(),
                "select secondary four".into(),
            ],
        ),
        (
            "Toggle Aim Assist",
            vec!["toggle aim assist".into(), "aim assist".into()],
        ),
        (
            "Next Ammunition",
            vec![
                "next ammunition".into(),
                "next ammo".into(),
                "cycle ammunition".into(),
            ],
        ),
        (
            "Deploy Countermeasures",
            vec![
                "deploy countermeasures".into(),
                "countermeasures".into(),
                "flares".into(),
                "chaff".into(),
            ],
        ),
        (
            "Map",
            vec!["map".into(), "galaxy map".into(), "open map".into()],
        ),
        ("Undock", vec!["undock".into(), "depart".into()]),
        ("Interact", vec!["interact".into(), "use".into()]),
        (
            "Get Up",
            vec!["get up".into(), "stand up".into(), "leave seat".into()],
        ),
        (
            "Next Target",
            vec!["next target".into(), "target next".into()],
        ),
        (
            "Previous Target",
            vec!["previous target".into(), "target previous".into()],
        ),
        (
            "Next Subcomponent",
            vec!["next subcomponent".into(), "target subcomponent".into()],
        ),
        (
            "Previous Subcomponent",
            vec!["previous subcomponent".into(), "previous subsystem".into()],
        ),
        (
            "Toggle Mouse Cursor",
            vec!["mouse cursor".into(), "toggle mouse".into()],
        ),
        (
            "Direct Mouse Mode",
            vec!["direct mouse mode".into(), "mouse flight".into()],
        ),
        ("Steering Mode", vec!["steering mode".into()]),
        (
            "Request Docking",
            vec![
                "request docking".into(),
                "dock".into(),
                "permission to dock".into(),
            ],
        ),
        (
            "Target View",
            vec!["target view".into(), "view target".into()],
        ),
        (
            "External View",
            vec!["external view".into(), "third person".into()],
        ),
        (
            "Cockpit View",
            vec![
                "cockpit view".into(),
                "first person".into(),
                "reset view".into(),
            ],
        ),
        (
            "Collect Loot",
            vec!["collect loot".into(), "drones collect".into()],
        ),
        (
            "Drones Attack",
            vec!["drones attack".into(), "attack my target".into()],
        ),
        ("Help", vec!["help".into()]),
        ("Info", vec!["info".into(), "information".into()]),
        (
            "Player Menu",
            vec!["player menu".into(), "my empire".into()],
        ),
        (
            "Mission Manager",
            vec!["missions".into(), "mission manager".into()],
        ),
    ];

    for (friendly, triggers) in voice_commands_data {
        profile
            .voice_commands
            .insert(friendly.to_string(), triggers);
    }

    // Macros for X4 (Parity Strike)
    profile.macros = vec![
        Macro {
            name: "EscapeVector".to_string(),
            triggers: vec!["escape".into(), "emergency escape".into(), "flee".into()],
            steps: vec![
                MacroStep {
                    action: "Boost".into(),
                    delay: 0,
                    ..Default::default()
                },
                MacroStep {
                    action: "Travel Mode".into(),
                    delay: 200,
                    ..Default::default()
                },
                MacroStep {
                    action: "Boost".into(),
                    delay: 1000,
                    ..Default::default()
                },
            ],
        },
        Macro {
            name: "ScanSurroundings".to_string(),
            triggers: vec!["scan sector".into(), "scan area".into()],
            steps: vec![
                MacroStep {
                    action: "Scan Mode".into(),
                    delay: 0,
                    ..Default::default()
                },
                MacroStep {
                    action: "Long Range Scan".into(),
                    delay: 1500,
                    ..Default::default()
                },
            ],
        },
        Macro {
            name: "CombatReady".to_string(),
            triggers: vec![
                "combat ready".into(),
                "battle stations".into(),
                "prepare for combat".into(),
            ],
            steps: vec![
                MacroStep {
                    action: "Cockpit View".into(),
                    delay: 0,
                    ..Default::default()
                },
                MacroStep {
                    action: "Fire Primary".into(),
                    delay: 100,
                    ..Default::default()
                },
                MacroStep {
                    action: "Next Target".into(),
                    delay: 500,
                    ..Default::default()
                },
            ],
        },
        Macro {
            name: "DockingProcedure".to_string(),
            triggers: vec![
                "docking procedure".into(),
                "prepare to dock".into(),
                "dock now".into(),
            ],
            steps: vec![
                MacroStep {
                    action: "Stop Engines".into(),
                    delay: 0,
                    ..Default::default()
                },
                MacroStep {
                    action: "Request Docking".into(),
                    delay: 1000,
                    ..Default::default()
                },
                MacroStep {
                    action: "Lower Shields".into(),
                    delay: 500,
                    ..Default::default()
                },
            ],
        },
        Macro {
            name: "EmergencyRetreat".to_string(),
            triggers: vec![
                "emergency retreat".into(),
                "get out of here".into(),
                "escape now".into(),
            ],
            steps: vec![
                MacroStep {
                    action: "Travel Mode".into(),
                    delay: 0,
                    ..Default::default()
                },
                MacroStep {
                    action: "Boost".into(),
                    delay: 500,
                    ..Default::default()
                },
            ],
        },
    ];

    // Process names for auto-detection (Extended for Parity)
    profile.process_names = vec![
        "X4.exe".into(),
        "X4".into(),
        "./X4".into(),
        "x4start.sh".into(),
        "Main".into(),
    ];

    // Path discriminators for Proton/GOG parity
    profile.path_discriminators = vec!["steamapps".into(), "compatdata".into(), "gog".into()];
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::GameType;

    #[test]
    fn test_parse_x4_input() {
        assert_eq!(parse_x4_input("INPUT_KEY_A"), Some("A".to_string()));
        assert_eq!(parse_x4_input("INPUT_KEY_SPACE"), Some("SPACE".to_string()));
        assert_eq!(parse_x4_input("INPUT_KEY_ESCAPE"), Some("ESC".to_string()));
        assert_eq!(parse_x4_input("INVALID"), None);
    }

    #[test]
    fn test_init_defaults() {
        let profile = GameProfile::new("Test X4", GameType::X4Foundations);
        assert!(profile.virtual_tags.contains_key("Boost"));
        assert!(profile.voice_commands.contains_key("Boost"));
        assert!(profile.process_names.contains(&"X4.exe".to_string()));
    }

    #[test]
    fn test_profile_resolve_actions() {
        let mut profile = GameProfile::new("Test X4", GameType::X4Foundations);

        // Insert a mock binding
        profile.raw_bindings.insert(
            "INPUT_ACTION_BOOST".to_string(),
            KeyBinding {
                action: "INPUT_ACTION_BOOST".to_string(),
                primary_key: Some("TAB".to_string()),
                secondary_key: None,
                modifiers: vec![],
            },
        );

        let actions = profile.resolve_actions();
        assert!(actions.contains_key("Boost"));
        assert_eq!(
            actions.get("Boost").unwrap().primary_key,
            Some("TAB".to_string())
        );
    }
}
