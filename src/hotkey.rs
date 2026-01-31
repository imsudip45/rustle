//! Global hotkey registration for Rustle
//!
//! This module handles registering and managing global hotkeys
//! that work system-wide, even when Rustle is not focused.
//! Uses the Windows RegisterHotKey API.

#![allow(dead_code)]

use crate::error::{Result, RustleError};
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
    MOD_SHIFT, MOD_WIN, VIRTUAL_KEY, VK_SPACE,
};

/// Unique identifier for our hotkey
/// Windows requires a unique ID for each registered hotkey
const HOTKEY_ID: i32 = 1;

/// Global flag to track if hotkey is registered
static HOTKEY_REGISTERED: AtomicBool = AtomicBool::new(false);

/// Hotkey manager for registering and unregistering global hotkeys
///
/// This struct manages the lifecycle of a global hotkey registration.
/// The hotkey is automatically unregistered when this struct is dropped.
pub struct HotkeyManager {
    /// Window handle that will receive WM_HOTKEY messages
    hwnd: HWND,

    /// The hotkey ID (for unregistration)
    id: i32,

    /// Whether a hotkey is currently registered
    registered: bool,
}

impl HotkeyManager {
    /// Creates a new HotkeyManager for the given window
    ///
    /// # Arguments
    /// * `hwnd` - Window handle that will receive hotkey messages
    ///
    /// # Returns
    /// A new HotkeyManager instance (no hotkey registered yet)
    pub fn new(hwnd: HWND) -> Self {
        Self {
            hwnd,
            id: HOTKEY_ID,
            registered: false,
        }
    }

    /// Registers the default hotkey (Alt + Space)
    ///
    /// Note: Win + Space is reserved by Windows for keyboard layout switching,
    /// so we use Alt + Space as the default.
    ///
    /// # Returns
    /// * `Ok(())` if registration succeeded
    /// * `Err(RustleError)` if registration failed
    pub fn register_default(&mut self) -> Result<()> {
        self.register(Modifier::Alt, Key::Space)
    }

    /// Registers a global hotkey with the specified modifier and key
    ///
    /// # Arguments
    /// * `modifier` - The modifier key (Win, Alt, Ctrl, Shift)
    /// * `key` - The main key
    ///
    /// # Returns
    /// * `Ok(())` if registration succeeded
    /// * `Err(RustleError)` if registration failed (e.g., key already in use)
    pub fn register(&mut self, modifier: Modifier, key: Key) -> Result<()> {
        // Unregister existing hotkey first
        if self.registered {
            self.unregister()?;
        }

        let mod_flags = modifier.to_windows_flags() | MOD_NOREPEAT;
        let vk_code = key.to_virtual_key();

        log::info!(
            "Registering hotkey: {:?} + {:?} (mod: {:?}, vk: {})",
            modifier,
            key,
            mod_flags,
            vk_code.0
        );

        let result = unsafe { RegisterHotKey(self.hwnd, self.id, mod_flags, vk_code.0 as u32) };

        if result.is_ok() {
            self.registered = true;
            HOTKEY_REGISTERED.store(true, Ordering::SeqCst);
            log::info!("Hotkey registered successfully");
            Ok(())
        } else {
            let error = windows::core::Error::from_win32();
            log::error!("Failed to register hotkey: {:?}", error);
            Err(RustleError::hotkey_registration(format!(
                "Failed to register hotkey: {:?}. The key combination may already be in use.",
                error
            )))
        }
    }

    /// Unregisters the current hotkey
    ///
    /// # Returns
    /// * `Ok(())` if unregistration succeeded
    /// * `Err(RustleError)` if unregistration failed
    pub fn unregister(&mut self) -> Result<()> {
        if !self.registered {
            return Ok(());
        }

        let result = unsafe { UnregisterHotKey(self.hwnd, self.id) };

        if result.is_ok() {
            self.registered = false;
            HOTKEY_REGISTERED.store(false, Ordering::SeqCst);
            log::info!("Hotkey unregistered successfully");
            Ok(())
        } else {
            let error = windows::core::Error::from_win32();
            Err(RustleError::HotkeyUnregistration(format!(
                "Failed to unregister hotkey: {:?}",
                error
            )))
        }
    }

    /// Returns the hotkey ID
    pub fn id(&self) -> i32 {
        self.id
    }

    /// Checks if a hotkey is currently registered
    pub fn is_registered(&self) -> bool {
        self.registered
    }
}

impl Drop for HotkeyManager {
    fn drop(&mut self) {
        if self.registered {
            if let Err(e) = self.unregister() {
                log::warn!("Failed to unregister hotkey on drop: {}", e);
            }
        }
    }
}

/// Modifier keys for hotkey combinations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modifier {
    /// Windows key
    Win,
    /// Alt key
    Alt,
    /// Control key
    Ctrl,
    /// Shift key
    Shift,
    /// Windows + Alt
    WinAlt,
    /// Control + Alt
    CtrlAlt,
    /// Control + Shift
    CtrlShift,
}

impl Modifier {
    /// Converts to Windows API modifier flags
    fn to_windows_flags(self) -> HOT_KEY_MODIFIERS {
        match self {
            Modifier::Win => MOD_WIN,
            Modifier::Alt => MOD_ALT,
            Modifier::Ctrl => MOD_CONTROL,
            Modifier::Shift => MOD_SHIFT,
            Modifier::WinAlt => MOD_WIN | MOD_ALT,
            Modifier::CtrlAlt => MOD_CONTROL | MOD_ALT,
            Modifier::CtrlShift => MOD_CONTROL | MOD_SHIFT,
        }
    }

    /// Parses a modifier from a string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "win" | "windows" | "super" => Some(Modifier::Win),
            "alt" => Some(Modifier::Alt),
            "ctrl" | "control" => Some(Modifier::Ctrl),
            "shift" => Some(Modifier::Shift),
            _ => None,
        }
    }
}

/// Keys that can be used in hotkey combinations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// Spacebar
    Space,
    /// Letter keys A-Z
    Letter(char),
    /// Function keys F1-F12
    Function(u8),
}

impl Key {
    /// Converts to Windows virtual key code
    fn to_virtual_key(self) -> VIRTUAL_KEY {
        match self {
            Key::Space => VK_SPACE,
            Key::Letter(c) => {
                // A-Z are virtual key codes 0x41-0x5A
                let upper = c.to_ascii_uppercase();
                VIRTUAL_KEY(upper as u16)
            }
            Key::Function(n) => {
                // F1-F12 are 0x70-0x7B
                VIRTUAL_KEY(0x70 + (n.saturating_sub(1) as u16))
            }
        }
    }

    /// Parses a key from a string
    pub fn from_str(s: &str) -> Option<Self> {
        let lower = s.to_lowercase();

        if lower == "space" {
            return Some(Key::Space);
        }

        // Check for function keys
        if lower.starts_with('f') {
            if let Ok(n) = lower[1..].parse::<u8>() {
                if (1..=12).contains(&n) {
                    return Some(Key::Function(n));
                }
            }
        }

        // Check for single letter
        if s.len() == 1 {
            let c = s.chars().next()?;
            if c.is_ascii_alphabetic() {
                return Some(Key::Letter(c));
            }
        }

        None
    }
}

/// Checks if the global hotkey is currently registered
pub fn is_hotkey_registered() -> bool {
    HOTKEY_REGISTERED.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modifier_from_str() {
        assert_eq!(Modifier::from_str("win"), Some(Modifier::Win));
        assert_eq!(Modifier::from_str("Win"), Some(Modifier::Win));
        assert_eq!(Modifier::from_str("alt"), Some(Modifier::Alt));
        assert_eq!(Modifier::from_str("ctrl"), Some(Modifier::Ctrl));
        assert_eq!(Modifier::from_str("invalid"), None);
    }

    #[test]
    fn test_key_from_str() {
        assert_eq!(Key::from_str("space"), Some(Key::Space));
        assert_eq!(Key::from_str("Space"), Some(Key::Space));
        assert_eq!(Key::from_str("j"), Some(Key::Letter('j')));
        assert_eq!(Key::from_str("F1"), Some(Key::Function(1)));
        assert_eq!(Key::from_str("F12"), Some(Key::Function(12)));
        assert_eq!(Key::from_str("invalid"), None);
    }

    #[test]
    fn test_key_virtual_key() {
        assert_eq!(Key::Space.to_virtual_key(), VK_SPACE);
        assert_eq!(Key::Letter('A').to_virtual_key(), VIRTUAL_KEY(0x41));
        assert_eq!(Key::Function(1).to_virtual_key(), VIRTUAL_KEY(0x70));
    }
}
