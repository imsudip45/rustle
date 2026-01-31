//! Configuration management for Rustle
//!
//! This module handles application configuration, including default values
//! and potential future support for user configuration files.

#![allow(dead_code)]

use std::path::PathBuf;

/// Application configuration
///
/// Contains all configurable settings for Rustle, with sensible defaults.
#[derive(Debug, Clone)]
pub struct Config {
    /// Hotkey configuration
    pub hotkey: HotkeyConfig,

    /// Search configuration
    pub search: SearchConfig,

    /// Appearance configuration
    pub appearance: AppearanceConfig,
}

/// Hotkey configuration settings
#[derive(Debug, Clone)]
pub struct HotkeyConfig {
    /// Modifier key (e.g., "win", "alt", "ctrl")
    pub modifier: String,

    /// Main key (e.g., "space", "j", "k")
    pub key: String,
}

/// Search behavior configuration
#[derive(Debug, Clone)]
pub struct SearchConfig {
    /// Maximum number of results to display
    pub max_results: usize,

    /// Whether to include hidden files in search
    pub include_hidden: bool,

    /// Directories to search for files
    pub search_paths: Vec<PathBuf>,

    /// File extensions to include (empty means all)
    pub file_extensions: Vec<String>,

    /// Maximum depth for directory traversal
    pub max_depth: usize,
}

/// UI appearance configuration
#[derive(Debug, Clone)]
pub struct AppearanceConfig {
    /// Window width in pixels
    pub width: u32,

    /// Window height in pixels (dynamic based on results)
    pub base_height: u32,

    /// Height of each result item
    pub item_height: u32,

    /// Window opacity (0.0 - 1.0)
    pub opacity: f32,

    /// Corner radius for rounded corners
    pub corner_radius: u32,

    /// Background color (ARGB)
    pub background_color: u32,

    /// Text color (ARGB)
    pub text_color: u32,

    /// Highlight color for selected item (ARGB)
    pub highlight_color: u32,

    /// Secondary text color (ARGB)
    pub secondary_text_color: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkey: HotkeyConfig::default(),
            search: SearchConfig::default(),
            appearance: AppearanceConfig::default(),
        }
    }
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            modifier: "alt".to_string(),
            key: "space".to_string(),
        }
    }
}

impl Default for SearchConfig {
    fn default() -> Self {
        // Get standard directories
        let mut search_paths = Vec::new();

        if let Some(docs) = dirs::document_dir() {
            search_paths.push(docs);
        }

        if let Some(downloads) = dirs::download_dir() {
            search_paths.push(downloads);
        }

        if let Some(desktop) = dirs::desktop_dir() {
            search_paths.push(desktop);
        }

        Self {
            max_results: 8,
            include_hidden: false,
            search_paths,
            file_extensions: Vec::new(), // All extensions
            max_depth: 5,
        }
    }
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            width: 680,
            base_height: 56,
            item_height: 48,
            opacity: 0.97,
            corner_radius: 12,
            // Windows 11 dark theme colors
            background_color: 0xFF2D2D2D,     // Dark gray background
            text_color: 0xFFFFFFFF,           // White text
            highlight_color: 0xFF3D3D3D,      // Slightly lighter for selection
            secondary_text_color: 0xFFAAAAAA, // Gray for paths/descriptions
        }
    }
}

impl Config {
    /// Creates a new configuration with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads configuration from the standard config location
    ///
    /// Falls back to defaults if config file doesn't exist or is invalid.
    pub fn load() -> Self {
        // For MVP, we just return defaults
        // Future: Load from %APPDATA%\rustle\config.toml
        if let Some(config_path) = Self::config_file_path() {
            if config_path.exists() {
                log::info!(
                    "Config file found at {:?}, using defaults for now",
                    config_path
                );
                // TODO: Parse TOML config file
            }
        }

        Self::default()
    }

    /// Returns the path to the configuration file
    pub fn config_file_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("rustle").join("config.toml"))
    }

    /// Returns the path to the data directory
    pub fn data_dir() -> Option<PathBuf> {
        dirs::data_dir().map(|p| p.join("rustle"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.hotkey.modifier, "alt");
        assert_eq!(config.hotkey.key, "space");
        assert_eq!(config.search.max_results, 8);
        assert!(config.appearance.opacity > 0.0);
    }

    #[test]
    fn test_search_paths_populated() {
        let config = SearchConfig::default();
        // Should have at least some paths on a Windows system
        // This test may vary based on the system
        assert!(config.max_depth > 0);
    }
}
