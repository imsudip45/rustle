//! Error types for Rustle
//!
//! This module defines custom error types used throughout the application,
//! providing clear error messages and proper error handling patterns.

#![allow(dead_code)]

use std::path::PathBuf;
use thiserror::Error;

/// Result type alias using RustleError
pub type Result<T> = std::result::Result<T, RustleError>;

/// Main error type for Rustle operations
///
/// This enum covers all possible error conditions in the application,
/// with descriptive messages for debugging and user feedback.
#[derive(Error, Debug)]
pub enum RustleError {
    /// Failed to register global hotkey
    #[error("Failed to register global hotkey: {0}")]
    HotkeyRegistration(String),

    /// Failed to unregister global hotkey
    #[error("Failed to unregister global hotkey: {0}")]
    HotkeyUnregistration(String),

    /// Window creation failed
    #[error("Failed to create window: {0}")]
    WindowCreation(String),

    /// Window class registration failed
    #[error("Failed to register window class: {0}")]
    WindowClassRegistration(String),

    /// Search operation failed
    #[error("Search operation failed: {0}")]
    SearchError(String),

    /// Failed to access a directory
    #[error("Cannot access directory: {path}")]
    DirectoryAccess {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Failed to launch an application or file
    #[error("Failed to launch: {path}")]
    LaunchError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Windows API error
    #[error("Windows API error: {0}")]
    WindowsApi(#[from] windows::core::Error),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid path provided
    #[error("Invalid path: {0}")]
    InvalidPath(String),

    /// UTF-8 conversion error
    #[error("UTF-8 conversion error: {0}")]
    Utf8Error(String),
}

impl RustleError {
    /// Creates a new hotkey registration error with context
    pub fn hotkey_registration(msg: impl Into<String>) -> Self {
        Self::HotkeyRegistration(msg.into())
    }

    /// Creates a new window creation error with context
    pub fn window_creation(msg: impl Into<String>) -> Self {
        Self::WindowCreation(msg.into())
    }

    /// Creates a new search error with context
    pub fn search_error(msg: impl Into<String>) -> Self {
        Self::SearchError(msg.into())
    }

    /// Creates a new launch error for the given path
    pub fn launch_error(path: PathBuf, source: std::io::Error) -> Self {
        Self::LaunchError { path, source }
    }

    /// Creates a new directory access error
    pub fn directory_access(path: PathBuf, source: std::io::Error) -> Self {
        Self::DirectoryAccess { path, source }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let error = RustleError::hotkey_registration("Key already in use");
        assert!(error.to_string().contains("hotkey"));
    }

    #[test]
    fn test_error_creation() {
        let path = PathBuf::from("C:\\test.exe");
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let error = RustleError::launch_error(path.clone(), io_error);

        match error {
            RustleError::LaunchError { path: p, .. } => assert_eq!(p, path),
            _ => panic!("Wrong error type"),
        }
    }
}
