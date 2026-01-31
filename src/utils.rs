//! Utility functions for Rustle
//!
//! This module contains helper functions used throughout the application,
//! including string manipulation, path utilities, and Windows-specific helpers.

#![allow(dead_code)]

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use unicode_normalization::UnicodeNormalization;

/// Converts a Rust string to a null-terminated wide string (UTF-16)
///
/// This is required for many Windows API calls that expect LPCWSTR.
///
/// # Arguments
/// * `s` - The string to convert
///
/// # Returns
/// A Vec<u16> containing the UTF-16 encoded string with null terminator
///
/// # Example
/// ```
/// use rustle::utils::to_wide_string;
/// let wide = to_wide_string("Hello");
/// assert_eq!(wide.last(), Some(&0u16));
/// ```
pub fn to_wide_string(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Converts a wide string (UTF-16) to a Rust String
///
/// # Arguments
/// * `wide` - The wide string slice to convert
///
/// # Returns
/// A String, or an empty string if conversion fails
pub fn from_wide_string(wide: &[u16]) -> String {
    // Find null terminator
    let len = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
    String::from_utf16_lossy(&wide[..len])
}

/// Normalizes a string for case-insensitive, accent-insensitive comparison
///
/// This function:
/// 1. Converts to lowercase
/// 2. Applies Unicode NFD normalization
/// 3. Removes diacritical marks
///
/// # Arguments
/// * `s` - The string to normalize
///
/// # Returns
/// A normalized string suitable for fuzzy matching
pub fn normalize_for_search(s: &str) -> String {
    s.to_lowercase()
        .nfd()
        // Filter out combining marks (diacritics) - they have Unicode category "Mn"
        // Keep alphanumeric chars and whitespace only
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect()
}

/// Extracts the file name without extension from a path
///
/// # Arguments
/// * `path` - The path to extract from
///
/// # Returns
/// The file stem as a string, or empty string if extraction fails
pub fn file_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string()
}

/// Extracts the file extension from a path
///
/// # Arguments
/// * `path` - The path to extract from
///
/// # Returns
/// The extension as a string (without the dot), or empty string if none
pub fn file_extension(path: &Path) -> String {
    path.extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase()
}

/// Truncates a string to a maximum length, adding ellipsis if needed
///
/// # Arguments
/// * `s` - The string to truncate
/// * `max_len` - Maximum length (including ellipsis)
///
/// # Returns
/// The truncated string
pub fn truncate_with_ellipsis(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len <= 3 {
        "...".to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

/// Formats a file size in bytes to a human-readable string
///
/// # Arguments
/// * `bytes` - The size in bytes
///
/// # Returns
/// A formatted string like "1.5 MB" or "256 KB"
pub fn format_file_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Checks if a file is likely an executable
///
/// # Arguments
/// * `path` - The path to check
///
/// # Returns
/// true if the file has an executable extension
pub fn is_executable(path: &Path) -> bool {
    matches!(
        file_extension(path).as_str(),
        "exe" | "msi" | "bat" | "cmd" | "ps1" | "com"
    )
}

/// Checks if a path is a Windows shortcut (.lnk file)
///
/// # Arguments
/// * `path` - The path to check
///
/// # Returns
/// true if the file is a .lnk shortcut
pub fn is_shortcut(path: &Path) -> bool {
    file_extension(path) == "lnk"
}

/// Gets the display name for a search result
///
/// For shortcuts, removes the .lnk extension.
/// For other files, shows the full filename.
///
/// # Arguments
/// * `path` - The path to get display name for
///
/// # Returns
/// A user-friendly display name
pub fn display_name(path: &Path) -> String {
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");

    // Remove .lnk extension for shortcuts
    if is_shortcut(path) {
        name.strip_suffix(".lnk")
            .or_else(|| name.strip_suffix(".LNK"))
            .unwrap_or(name)
            .to_string()
    } else {
        name.to_string()
    }
}

/// Gets the parent directory name for display
///
/// # Arguments
/// * `path` - The path to extract parent from
///
/// # Returns
/// The parent folder name, or empty string
pub fn parent_folder_name(path: &Path) -> String {
    path.parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_to_wide_string() {
        let wide = to_wide_string("Test");
        assert_eq!(wide.len(), 5); // 4 chars + null terminator
        assert_eq!(wide[4], 0);
    }

    #[test]
    fn test_from_wide_string() {
        let wide: Vec<u16> = vec![72, 105, 0]; // "Hi"
        assert_eq!(from_wide_string(&wide), "Hi");
    }

    #[test]
    fn test_normalize_for_search() {
        assert_eq!(normalize_for_search("HELLO"), "hello");
        assert_eq!(normalize_for_search("Café"), "cafe");
    }

    #[test]
    fn test_file_stem() {
        let path = PathBuf::from("C:\\test\\file.txt");
        assert_eq!(file_stem(&path), "file");
    }

    #[test]
    fn test_truncate_with_ellipsis() {
        assert_eq!(truncate_with_ellipsis("Hello World", 8), "Hello...");
        assert_eq!(truncate_with_ellipsis("Hi", 10), "Hi");
    }

    #[test]
    fn test_format_file_size() {
        assert_eq!(format_file_size(500), "500 B");
        assert_eq!(format_file_size(1536), "2 KB");
        assert_eq!(format_file_size(1_500_000), "1.4 MB");
    }

    #[test]
    fn test_is_shortcut() {
        assert!(is_shortcut(Path::new("app.lnk")));
        assert!(!is_shortcut(Path::new("app.exe")));
    }

    #[test]
    fn test_display_name() {
        assert_eq!(display_name(Path::new("Chrome.lnk")), "Chrome");
        assert_eq!(display_name(Path::new("file.txt")), "file.txt");
    }
}
