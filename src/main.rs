//! Rustle - A fast, lightweight Windows 11 search widget
//!
//! Rustle is a keyboard-driven application launcher and file finder that
//! provides instant search results for installed applications and files.
//!
//! ## Features
//!
//! - Global hotkey (Win + Space) to open from anywhere
//! - Fuzzy search for applications and files
//! - Keyboard navigation with arrow keys
//! - Instant launch with Enter key
//! - Minimalist Windows 11 style UI
//!
//! ## Usage
//!
//! ```bash
//! rustle.exe
//! ```
//!
//! Press Win + Space to open the search overlay, type to search,
//! use arrow keys to navigate, and press Enter to launch.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Modules
mod clipboard;
mod config;
mod error;
mod hotkey;
mod icons;
mod launcher;
mod search;
mod utils;
mod window;

use config::Config;
use error::Result;
use search::SearchEngine;

/// Application entry point
///
/// Initializes logging, loads configuration, creates the search engine,
/// and starts the main window event loop.
fn main() {
    // Initialize logging
    init_logging();

    log::info!("Rustle starting up...");

    // Run the application
    if let Err(e) = run() {
        log::error!("Application error: {}", e);
        show_error_dialog(&format!("Rustle encountered an error:\n\n{}", e));
        std::process::exit(1);
    }

    log::info!("Rustle shutting down.");
}

/// Main application logic
///
/// Separated from main() for proper error handling.
fn run() -> Result<()> {
    // Load configuration
    let config = Config::load();
    log::info!("Configuration loaded");

    // Create search engine and index applications
    log::info!("Indexing applications...");
    let search_engine = SearchEngine::new(config.search.clone());
    log::info!("Indexed {} applications", search_engine.application_count());

    // Log search paths
    for path in &config.search.search_paths {
        log::debug!("Search path: {:?}", path);
    }

    // Create and run the main window
    log::info!("Creating main window...");
    log::info!("Press Alt + Space to open Rustle");

    window::create_and_run(search_engine, config.appearance)?;

    Ok(())
}

/// Initializes the logging system
///
/// Uses env_logger with a custom format.
/// Set RUST_LOG environment variable to control log level.
fn init_logging() {
    use std::io::Write;

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buf, record| {
            let level_style = match record.level() {
                log::Level::Error => "\x1b[31m", // Red
                log::Level::Warn => "\x1b[33m",  // Yellow
                log::Level::Info => "\x1b[32m",  // Green
                log::Level::Debug => "\x1b[36m", // Cyan
                log::Level::Trace => "\x1b[90m", // Gray
            };

            writeln!(
                buf,
                "{}{:5}\x1b[0m {} - {}",
                level_style,
                record.level(),
                record.target(),
                record.args()
            )
        })
        .init();
}

/// Shows an error dialog to the user
///
/// Uses Windows MessageBox for displaying errors.
fn show_error_dialog(message: &str) {
    use crate::utils::to_wide_string;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

    let title = to_wide_string("Rustle Error");
    let text = to_wide_string(message);

    unsafe {
        MessageBoxW(
            HWND::default(),
            windows::core::PCWSTR(text.as_ptr()),
            windows::core::PCWSTR(title.as_ptr()),
            MB_ICONERROR | MB_OK,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_loads() {
        let config = Config::load();
        assert!(!config.hotkey.key.is_empty());
    }
}
