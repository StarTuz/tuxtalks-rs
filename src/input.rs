//! Input simulation module using Linux evdev/uinput
//!
//! Provides native key simulation without X11 dependencies.
//! Works on both X11 and Wayland.

use anyhow::{Context, Result};
use evdev::{uinput::VirtualDeviceBuilder, AttributeSet, Key};
use std::thread;
use std::time::Duration;
use tracing::{debug, info};

/// Virtual keyboard for simulating key presses
pub struct VirtualKeyboard {
    device: evdev::uinput::VirtualDevice,
}

impl VirtualKeyboard {
    /// Create a new virtual keyboard device
    pub fn new() -> Result<Self> {
        // Define which keys we can simulate
        let mut keys = AttributeSet::<Key>::new();

        // Add common keys
        for key in [
            // Letters
            Key::KEY_A,
            Key::KEY_B,
            Key::KEY_C,
            Key::KEY_D,
            Key::KEY_E,
            Key::KEY_F,
            Key::KEY_G,
            Key::KEY_H,
            Key::KEY_I,
            Key::KEY_J,
            Key::KEY_K,
            Key::KEY_L,
            Key::KEY_M,
            Key::KEY_N,
            Key::KEY_O,
            Key::KEY_P,
            Key::KEY_Q,
            Key::KEY_R,
            Key::KEY_S,
            Key::KEY_T,
            Key::KEY_U,
            Key::KEY_V,
            Key::KEY_W,
            Key::KEY_X,
            Key::KEY_Y,
            Key::KEY_Z,
            // Numbers
            Key::KEY_0,
            Key::KEY_1,
            Key::KEY_2,
            Key::KEY_3,
            Key::KEY_4,
            Key::KEY_5,
            Key::KEY_6,
            Key::KEY_7,
            Key::KEY_8,
            Key::KEY_9,
            // Function keys
            Key::KEY_F1,
            Key::KEY_F2,
            Key::KEY_F3,
            Key::KEY_F4,
            Key::KEY_F5,
            Key::KEY_F6,
            Key::KEY_F7,
            Key::KEY_F8,
            Key::KEY_F9,
            Key::KEY_F10,
            Key::KEY_F11,
            Key::KEY_F12,
            // Modifiers
            Key::KEY_LEFTSHIFT,
            Key::KEY_RIGHTSHIFT,
            Key::KEY_LEFTCTRL,
            Key::KEY_RIGHTCTRL,
            Key::KEY_LEFTALT,
            Key::KEY_RIGHTALT,
            // Navigation
            Key::KEY_UP,
            Key::KEY_DOWN,
            Key::KEY_LEFT,
            Key::KEY_RIGHT,
            Key::KEY_HOME,
            Key::KEY_END,
            Key::KEY_PAGEUP,
            Key::KEY_PAGEDOWN,
            // Common
            Key::KEY_SPACE,
            Key::KEY_ENTER,
            Key::KEY_TAB,
            Key::KEY_ESC,
            Key::KEY_BACKSPACE,
            Key::KEY_DELETE,
            // Gaming common
            Key::KEY_INSERT,
            Key::KEY_PAUSE,
        ] {
            keys.insert(key);
        }

        let device = VirtualDeviceBuilder::new()?
            .name("TuxTalks Virtual Keyboard")
            .with_keys(&keys)?
            .build()
            .context("Failed to create virtual keyboard")?;

        info!("⌨️ Virtual keyboard created");
        Ok(Self { device })
    }

    /// Press and release a single key
    pub fn tap_key(&mut self, key: Key) -> Result<()> {
        self.press_key(key)?;
        thread::sleep(Duration::from_millis(10));
        self.release_key(key)?;
        Ok(())
    }

    /// Press a key (without releasing)
    pub fn press_key(&mut self, key: Key) -> Result<()> {
        debug!("Key down: {:?}", key);
        self.device.emit(&[evdev::InputEvent::new(
            evdev::EventType::KEY,
            key.code(),
            1, // Press
        )])?;
        Ok(())
    }

    /// Release a key
    pub fn release_key(&mut self, key: Key) -> Result<()> {
        debug!("Key up: {:?}", key);
        self.device.emit(&[evdev::InputEvent::new(
            evdev::EventType::KEY,
            key.code(),
            0, // Release
        )])?;
        Ok(())
    }

    /// Type a key combination (e.g., Ctrl+C)
    pub fn key_combo(&mut self, modifiers: &[Key], key: Key) -> Result<()> {
        // Press modifiers
        for modifier in modifiers {
            self.press_key(*modifier)?;
            thread::sleep(Duration::from_millis(5));
        }

        // Tap the main key
        self.tap_key(key)?;

        // Release modifiers in reverse order
        for modifier in modifiers.iter().rev() {
            self.release_key(*modifier)?;
            thread::sleep(Duration::from_millis(5));
        }

        Ok(())
    }
}

/// Parse a key name string to evdev Key
pub fn parse_key(name: &str) -> Option<Key> {
    match name.to_uppercase().as_str() {
        // Letters
        "A" => Some(Key::KEY_A),
        "B" => Some(Key::KEY_B),
        "C" => Some(Key::KEY_C),
        "D" => Some(Key::KEY_D),
        "E" => Some(Key::KEY_E),
        "F" => Some(Key::KEY_F),
        "G" => Some(Key::KEY_G),
        "H" => Some(Key::KEY_H),
        "I" => Some(Key::KEY_I),
        "J" => Some(Key::KEY_J),
        "K" => Some(Key::KEY_K),
        "L" => Some(Key::KEY_L),
        "M" => Some(Key::KEY_M),
        "N" => Some(Key::KEY_N),
        "O" => Some(Key::KEY_O),
        "P" => Some(Key::KEY_P),
        "Q" => Some(Key::KEY_Q),
        "R" => Some(Key::KEY_R),
        "S" => Some(Key::KEY_S),
        "T" => Some(Key::KEY_T),
        "U" => Some(Key::KEY_U),
        "V" => Some(Key::KEY_V),
        "W" => Some(Key::KEY_W),
        "X" => Some(Key::KEY_X),
        "Y" => Some(Key::KEY_Y),
        "Z" => Some(Key::KEY_Z),
        // Numbers
        "0" => Some(Key::KEY_0),
        "1" => Some(Key::KEY_1),
        "2" => Some(Key::KEY_2),
        "3" => Some(Key::KEY_3),
        "4" => Some(Key::KEY_4),
        "5" => Some(Key::KEY_5),
        "6" => Some(Key::KEY_6),
        "7" => Some(Key::KEY_7),
        "8" => Some(Key::KEY_8),
        "9" => Some(Key::KEY_9),
        // Function keys
        "F1" => Some(Key::KEY_F1),
        "F2" => Some(Key::KEY_F2),
        "F3" => Some(Key::KEY_F3),
        "F4" => Some(Key::KEY_F4),
        "F5" => Some(Key::KEY_F5),
        "F6" => Some(Key::KEY_F6),
        "F7" => Some(Key::KEY_F7),
        "F8" => Some(Key::KEY_F8),
        "F9" => Some(Key::KEY_F9),
        "F10" => Some(Key::KEY_F10),
        "F11" => Some(Key::KEY_F11),
        "F12" => Some(Key::KEY_F12),
        // Modifiers
        "SHIFT" | "LSHIFT" => Some(Key::KEY_LEFTSHIFT),
        "RSHIFT" => Some(Key::KEY_RIGHTSHIFT),
        "CTRL" | "LCTRL" | "CONTROL" => Some(Key::KEY_LEFTCTRL),
        "RCTRL" => Some(Key::KEY_RIGHTCTRL),
        "ALT" | "LALT" => Some(Key::KEY_LEFTALT),
        "RALT" => Some(Key::KEY_RIGHTALT),
        // Navigation
        "UP" => Some(Key::KEY_UP),
        "DOWN" => Some(Key::KEY_DOWN),
        "LEFT" => Some(Key::KEY_LEFT),
        "RIGHT" => Some(Key::KEY_RIGHT),
        "HOME" => Some(Key::KEY_HOME),
        "END" => Some(Key::KEY_END),
        "PAGEUP" | "PGUP" => Some(Key::KEY_PAGEUP),
        "PAGEDOWN" | "PGDN" => Some(Key::KEY_PAGEDOWN),
        // Common
        "SPACE" => Some(Key::KEY_SPACE),
        "ENTER" | "RETURN" => Some(Key::KEY_ENTER),
        "TAB" => Some(Key::KEY_TAB),
        "ESC" | "ESCAPE" => Some(Key::KEY_ESC),
        "BACKSPACE" => Some(Key::KEY_BACKSPACE),
        "DELETE" | "DEL" => Some(Key::KEY_DELETE),
        "INSERT" | "INS" => Some(Key::KEY_INSERT),
        "PAUSE" => Some(Key::KEY_PAUSE),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_key() {
        assert_eq!(parse_key("a"), Some(Key::KEY_A));
        assert_eq!(parse_key("A"), Some(Key::KEY_A));
        assert_eq!(parse_key("F1"), Some(Key::KEY_F1));
        assert_eq!(parse_key("space"), Some(Key::KEY_SPACE));
        assert_eq!(parse_key("unknown"), None);
    }
}
