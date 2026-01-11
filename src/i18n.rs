//! Internationalization (i18n) Support
//!
//! Provides translation functions for the TuxTalks GUI.
//! Based on Python i18n.py for feature parity.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;
use tracing::{debug, info};

/// Current active language
static CURRENT_LANG: RwLock<String> = RwLock::new(String::new());

/// Loaded translations (language -> key -> value)
static TRANSLATIONS: RwLock<Option<HashMap<String, HashMap<String, String>>>> = RwLock::new(None);

/// RTL (Right-to-Left) languages
const RTL_LANGUAGES: &[&str] = &["ar", "he", "fa", "ur"];

/// Initialize i18n system with locale directory
pub fn init(lang: Option<&str>) {
    let lang = lang.unwrap_or("en");
    set_language(lang);
}

/// Set the active language
pub fn set_language(lang: &str) {
    let mut current = CURRENT_LANG.write().expect("i18n lock poisoned");
    *current = lang.to_string();
    info!("🌐 Language set to: {}", lang);

    // Load translations for this language
    load_translations(lang);
}

/// Get the current language
pub fn current_language() -> String {
    CURRENT_LANG.read().expect("i18n lock poisoned").clone()
}

/// Check if current language is RTL
pub fn is_rtl() -> bool {
    let lang = current_language();
    RTL_LANGUAGES.contains(&lang.as_str())
}

/// Translate a key (gettext-style)
pub fn tr(key: &str) -> String {
    let lang = current_language();

    let translations = TRANSLATIONS.read().expect("i18n lock poisoned");
    if let Some(ref all_trans) = *translations {
        if let Some(lang_trans) = all_trans.get(&lang) {
            if let Some(value) = lang_trans.get(key) {
                return value.clone();
            }
        }
    }

    // Fallback to key itself
    key.to_string()
}

/// Load translations from locale directory
fn load_translations(lang: &str) {
    let locale_dirs = [
        dirs::data_local_dir().map(|p| p.join("tuxtalks/locale")),
        Some(PathBuf::from("/usr/local/share/locale")),
        Some(PathBuf::from("locale")),
    ];

    for maybe_dir in locale_dirs.iter().flatten() {
        let mo_path = maybe_dir.join(format!("{}/LC_MESSAGES/tuxtalks.json", lang));
        if mo_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&mo_path) {
                if let Ok(trans) = serde_json::from_str::<HashMap<String, String>>(&content) {
                    let mut all = TRANSLATIONS.write().expect("i18n lock poisoned");
                    if all.is_none() {
                        *all = Some(HashMap::new());
                    }
                    if let Some(ref mut map) = *all {
                        map.insert(lang.to_string(), trans);
                        debug!(
                            "Loaded {} translations for '{}'",
                            map.get(lang).map(|t| t.len()).unwrap_or(0),
                            lang
                        );
                        return;
                    }
                }
            }
        }
    }

    // No translations found - use English defaults
    debug!("No translations found for '{}', using English", lang);
}

/// Get text alignment for current language
pub fn text_align() -> &'static str {
    if is_rtl() {
        "right"
    } else {
        "left"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rtl_detection() {
        set_language("ar");
        assert!(is_rtl());

        set_language("en");
        assert!(!is_rtl());
    }

    #[test]
    fn test_translation_fallback() {
        set_language("en");
        // Unknown key returns itself
        assert_eq!(tr("unknown_key"), "unknown_key");
    }
}
