//! Input simulation and listening module using Linux evdev/uinput
//!
//! Provides native key simulation and global key listening without X11 dependencies.
//! Works on both X11 and Wayland.

use anyhow::{Context, Result};
use evdev::{uinput::VirtualDeviceBuilder, AttributeSet, EventType, InputEvent, Key};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
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
        self.device.emit(&[InputEvent::new(
            EventType::KEY,
            key.code(),
            1, // Press
        )])?;
        Ok(())
    }

    /// Release a key
    pub fn release_key(&mut self, key: Key) -> Result<()> {
        debug!("Key up: {:?}", key);
        self.device.emit(&[InputEvent::new(
            EventType::KEY,
            key.code(),
            0, // Release
        )])?;
        Ok(())
    }

    /// Type a key combination (e.g., Ctrl+C)
    pub fn key_combo(&mut self, modifiers: &[Key], key: Key) -> Result<()> {
        for modifier in modifiers {
            self.press_key(*modifier)?;
            thread::sleep(Duration::from_millis(5));
        }
        self.tap_key(key)?;
        for modifier in modifiers.iter().rev() {
            self.release_key(*modifier)?;
            thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    }
}

/// Listens for global key events using evdev
pub struct InputListener {
    ptt_key: Option<Key>,
    ptt_mode: PttMode,
    key_bindings: HashMap<Key, String>,
    state: Arc<Mutex<ListenerState>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PttMode {
    Hold,
    Toggle,
}

struct ListenerState {
    ptt_active: bool,
    last_toggle: Instant,
}

impl InputListener {
    pub fn new(ptt_key: Option<Key>, ptt_mode: PttMode) -> Self {
        Self {
            ptt_key,
            ptt_mode,
            key_bindings: HashMap::new(),
            state: Arc::new(Mutex::new(ListenerState {
                ptt_active: false,
                last_toggle: Instant::now() - Duration::from_secs(1),
            })),
        }
    }

    pub fn add_binding(&mut self, key: Key, command: String) {
        self.key_bindings.insert(key, command);
    }

    pub fn is_ptt_active(&self) -> bool {
        self.state.lock().map(|s| s.ptt_active).unwrap_or(false)
    }

    /// Starts listening in a background thread
    pub fn start(&mut self) -> Result<tokio::sync::mpsc::Receiver<String>> {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let ptt_key = self.ptt_key;
        let ptt_mode = self.ptt_mode;
        let bindings = self.key_bindings.clone();
        let state = Arc::clone(&self.state);

        thread::spawn(move || {
            let mut devices = Vec::new();

            loop {
                // Find keyboards if we have none
                if devices.is_empty() {
                    for (_, dev) in evdev::enumerate() {
                        if dev
                            .name()
                            .map(|n| n.to_lowercase().contains("keyboard"))
                            .unwrap_or(false)
                        {
                            devices.push(dev);
                        }
                    }
                    if devices.is_empty() {
                        thread::sleep(Duration::from_secs(2));
                        continue;
                    }
                    debug!("⌨️ Found {} keyboard device(s)", devices.len());
                }

                // Poll devices
                let mut to_remove = Vec::new();
                for (idx, dev) in devices.iter_mut().enumerate() {
                    match dev.fetch_events() {
                        Ok(events) => {
                            for event in events {
                                if event.event_type() == EventType::KEY {
                                    let key = Key::new(event.code());
                                    let val = event.value(); // 0: release, 1: press, 2: repeat

                                    // Handle PTT
                                    if Some(key) == ptt_key {
                                        let mut s =
                                            state.lock().expect("InputListener mutex poisoned");
                                        match ptt_mode {
                                            PttMode::Hold => {
                                                if val == 1 {
                                                    s.ptt_active = true;
                                                } else if val == 0 {
                                                    s.ptt_active = false;
                                                }
                                            }
                                            PttMode::Toggle => {
                                                if val == 1
                                                    && s.last_toggle.elapsed()
                                                        > Duration::from_millis(500)
                                                {
                                                    s.ptt_active = !s.ptt_active;
                                                    s.last_toggle = Instant::now();
                                                    info!("🎤 PTT Toggled: {}", s.ptt_active);
                                                }
                                            }
                                        }
                                    }

                                    // Handle bindings
                                    if val == 1 {
                                        if let Some(cmd) = bindings.get(&key) {
                                            let _ = tx.blocking_send(cmd.clone());
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                        Err(_) => {
                            to_remove.push(idx);
                        }
                    }
                }

                for idx in to_remove.into_iter().rev() {
                    devices.remove(idx);
                }

                thread::sleep(Duration::from_millis(10));
            }
        });

        Ok(rx)
    }
}

/// Parse a key name string to evdev Key
pub fn parse_key(name: &str) -> Option<Key> {
    let name_lower = name.to_lowercase();

    // 1. Explicit Scancode Mapping for Parity (Team APPROVED)
    // This ensures ydotool-style string digits result in correct hardware codes (e.g. '1' -> 2)
    // This is critical for Wayland stability and parity with the Python fix.
    let scancode = match name_lower.as_str() {
        "1" => Some(2),
        "2" => Some(3),
        "3" => Some(4),
        "4" => Some(5),
        "5" => Some(6),
        "6" => Some(7),
        "7" => Some(8),
        "8" => Some(9),
        "9" => Some(10),
        "0" => Some(11),
        "tab" => Some(15),
        "enter" | "return" => Some(28),
        "space" => Some(57),
        "esc" | "escape" => Some(1),
        "backspace" => Some(14),
        "delete" | "del" => Some(111),
        "up" => Some(103),
        "down" => Some(108),
        "left" => Some(105),
        "right" => Some(106),
        _ => None,
    };

    if let Some(code) = scancode {
        return Some(Key::new(code));
    }

    // 2. Legacy/Standard mapping
    // Add prefix if missing
    let full_name = if name.starts_with("KEY_") {
        name.to_uppercase()
    } else {
        format!("KEY_{}", name.to_uppercase())
    };

    match full_name.as_str() {
        "KEY_A" => Some(Key::KEY_A),
        "KEY_B" => Some(Key::KEY_B),
        "KEY_C" => Some(Key::KEY_C),
        "KEY_D" => Some(Key::KEY_D),
        "KEY_E" => Some(Key::KEY_E),
        "KEY_F" => Some(Key::KEY_F),
        "KEY_G" => Some(Key::KEY_G),
        "KEY_H" => Some(Key::KEY_H),
        "KEY_I" => Some(Key::KEY_I),
        "KEY_J" => Some(Key::KEY_J),
        "KEY_K" => Some(Key::KEY_K),
        "KEY_L" => Some(Key::KEY_L),
        "KEY_M" => Some(Key::KEY_M),
        "KEY_N" => Some(Key::KEY_N),
        "KEY_O" => Some(Key::KEY_O),
        "KEY_P" => Some(Key::KEY_P),
        "KEY_Q" => Some(Key::KEY_Q),
        "KEY_R" => Some(Key::KEY_R),
        "KEY_S" => Some(Key::KEY_S),
        "KEY_T" => Some(Key::KEY_T),
        "KEY_U" => Some(Key::KEY_U),
        "KEY_V" => Some(Key::KEY_V),
        "KEY_W" => Some(Key::KEY_W),
        "KEY_X" => Some(Key::KEY_X),
        "KEY_Y" => Some(Key::KEY_Y),
        "KEY_Z" => Some(Key::KEY_Z),
        "KEY_0" => Some(Key::KEY_0),
        "KEY_1" => Some(Key::KEY_1),
        "KEY_2" => Some(Key::KEY_2),
        "KEY_3" => Some(Key::KEY_3),
        "KEY_4" => Some(Key::KEY_4),
        "KEY_5" => Some(Key::KEY_5),
        "KEY_6" => Some(Key::KEY_6),
        "KEY_7" => Some(Key::KEY_7),
        "KEY_8" => Some(Key::KEY_8),
        "KEY_9" => Some(Key::KEY_9),
        "KEY_F1" => Some(Key::KEY_F1),
        "KEY_F2" => Some(Key::KEY_F2),
        "KEY_F3" => Some(Key::KEY_F3),
        "KEY_F4" => Some(Key::KEY_F4),
        "KEY_F5" => Some(Key::KEY_F5),
        "KEY_F6" => Some(Key::KEY_F6),
        "KEY_F7" => Some(Key::KEY_F7),
        "KEY_F8" => Some(Key::KEY_F8),
        "KEY_F9" => Some(Key::KEY_F9),
        "KEY_F10" => Some(Key::KEY_F10),
        "KEY_F11" => Some(Key::KEY_F11),
        "KEY_F12" => Some(Key::KEY_F12),
        "KEY_LEFTSHIFT" | "KEY_SHIFT" => Some(Key::KEY_LEFTSHIFT),
        "KEY_RIGHTSHIFT" => Some(Key::KEY_RIGHTSHIFT),
        "KEY_LEFTCTRL" | "KEY_CTRL" | "KEY_CONTROL" => Some(Key::KEY_LEFTCTRL),
        "KEY_RIGHTCTRL" => Some(Key::KEY_RIGHTCTRL),
        "KEY_LEFTALT" | "KEY_ALT" => Some(Key::KEY_LEFTALT),
        "KEY_RIGHTALT" => Some(Key::KEY_RIGHTALT),
        "KEY_UP" => Some(Key::KEY_UP),
        "KEY_DOWN" => Some(Key::KEY_DOWN),
        "KEY_LEFT" => Some(Key::KEY_LEFT),
        "KEY_RIGHT" => Some(Key::KEY_RIGHT),
        "KEY_HOME" => Some(Key::KEY_HOME),
        "KEY_END" => Some(Key::KEY_END),
        "KEY_PAGEUP" | "KEY_PGUP" => Some(Key::KEY_PAGEUP),
        "KEY_PAGEDOWN" | "KEY_PGDN" => Some(Key::KEY_PAGEDOWN),
        "KEY_SPACE" => Some(Key::KEY_SPACE),
        "KEY_ENTER" | "KEY_RETURN" => Some(Key::KEY_ENTER),
        "KEY_TAB" => Some(Key::KEY_TAB),
        "KEY_ESC" | "KEY_ESCAPE" => Some(Key::KEY_ESC),
        "KEY_BACKSPACE" => Some(Key::KEY_BACKSPACE),
        "KEY_DELETE" | "KEY_DEL" => Some(Key::KEY_DELETE),
        "KEY_INSERT" | "KEY_INS" => Some(Key::KEY_INSERT),
        "KEY_PAUSE" => Some(Key::KEY_PAUSE),
        _ => None,
    }
}
