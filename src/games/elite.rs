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

use super::KeyBinding;

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
                if let Some(ref action) = current_action {
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
        // Letters are already fine
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_key() {
        assert_eq!(normalize_ed_key("Key_A"), "A");
        assert_eq!(normalize_ed_key("Key_Space"), "SPACE");
        assert_eq!(normalize_ed_key("Key_LeftShift"), "LSHIFT");
        assert_eq!(normalize_ed_key("Key_F1"), "F1");
    }
}
