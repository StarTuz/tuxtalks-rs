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

use super::KeyBinding;

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

                // X4 uses <action> elements with id and input attributes
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

    #[test]
    fn test_parse_x4_input() {
        assert_eq!(parse_x4_input("INPUT_KEY_A"), Some("A".to_string()));
        assert_eq!(parse_x4_input("INPUT_KEY_SPACE"), Some("SPACE".to_string()));
        assert_eq!(parse_x4_input("INPUT_KEY_F1"), Some("F1".to_string()));
        assert_eq!(parse_x4_input("INVALID"), None);
    }
}
