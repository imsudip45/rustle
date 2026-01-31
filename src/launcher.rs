//! Application and file launcher for Rustle
//!
//! This module handles launching applications and opening files
//! using the Windows shell. It uses ShellExecuteW for maximum
//! compatibility with different file types.

#![allow(dead_code)]

use crate::error::{Result, RustleError};
use crate::utils::to_wide_string;
use std::path::Path;
use windows::core::PCWSTR;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// Launches an application or opens a file
///
/// Uses Windows ShellExecuteW to launch files, which handles:
/// - Executable files (.exe, .msi, .bat, etc.)
/// - Shortcut files (.lnk)
/// - Documents (opens with associated application)
/// - Folders (opens in Explorer)
///
/// # Arguments
/// * `path` - Path to the file or application to launch
///
/// # Returns
/// * `Ok(())` if the launch was initiated successfully
/// * `Err(RustleError)` if the launch failed
///
/// # Example
/// ```no_run
/// use rustle::launcher::launch;
/// use std::path::Path;
///
/// launch(Path::new(r"C:\Windows\notepad.exe")).unwrap();
/// ```
pub fn launch(path: &Path) -> Result<()> {
    // Validate path exists
    if !path.exists() {
        return Err(RustleError::InvalidPath(format!(
            "Path does not exist: {}",
            path.display()
        )));
    }

    log::info!("Launching: {}", path.display());

    // Convert path to wide string
    let path_wide = to_wide_string(&path.to_string_lossy());

    // "open" verb for ShellExecute
    let verb = to_wide_string("open");

    // Execute using Windows Shell
    let result = unsafe {
        ShellExecuteW(
            HWND::default(),            // No parent window
            PCWSTR(verb.as_ptr()),      // "open" verb
            PCWSTR(path_wide.as_ptr()), // File path
            PCWSTR::null(),             // No parameters
            PCWSTR::null(),             // No working directory (use default)
            SW_SHOWNORMAL,              // Show window normally
        )
    };

    // ShellExecuteW returns a value > 32 on success
    let result_code = result.0 as isize;

    if result_code > 32 {
        log::info!("Successfully launched: {}", path.display());
        Ok(())
    } else {
        let error_msg = shell_execute_error_message(result_code);
        log::error!(
            "Failed to launch {}: {} (code: {})",
            path.display(),
            error_msg,
            result_code
        );
        Err(RustleError::LaunchError {
            path: path.to_path_buf(),
            source: std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("ShellExecute failed: {} (code: {})", error_msg, result_code),
            ),
        })
    }
}

/// Launches a file with specific parameters
///
/// Similar to `launch` but allows passing command-line arguments.
///
/// # Arguments
/// * `path` - Path to the executable
/// * `args` - Command-line arguments
///
/// # Returns
/// * `Ok(())` if the launch was initiated successfully
/// * `Err(RustleError)` if the launch failed
pub fn launch_with_args(path: &Path, args: &str) -> Result<()> {
    if !path.exists() {
        return Err(RustleError::InvalidPath(format!(
            "Path does not exist: {}",
            path.display()
        )));
    }

    log::info!("Launching: {} with args: {}", path.display(), args);

    let path_wide = to_wide_string(&path.to_string_lossy());
    let verb = to_wide_string("open");
    let args_wide = to_wide_string(args);

    let result = unsafe {
        ShellExecuteW(
            HWND::default(),
            PCWSTR(verb.as_ptr()),
            PCWSTR(path_wide.as_ptr()),
            PCWSTR(args_wide.as_ptr()),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };

    let result_code = result.0 as isize;

    if result_code > 32 {
        Ok(())
    } else {
        Err(RustleError::LaunchError {
            path: path.to_path_buf(),
            source: std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("ShellExecute failed with code: {}", result_code),
            ),
        })
    }
}

/// Opens a folder in Windows Explorer
///
/// # Arguments
/// * `path` - Path to the folder to open
///
/// # Returns
/// * `Ok(())` if Explorer was opened successfully
/// * `Err(RustleError)` if the operation failed
pub fn open_folder(path: &Path) -> Result<()> {
    if !path.is_dir() {
        return Err(RustleError::InvalidPath(format!(
            "Path is not a directory: {}",
            path.display()
        )));
    }

    launch(path)
}

/// Opens the containing folder for a file and selects it
///
/// Uses Explorer's /select parameter to highlight the file.
///
/// # Arguments
/// * `path` - Path to the file whose folder should be opened
///
/// # Returns
/// * `Ok(())` if Explorer was opened successfully
/// * `Err(RustleError)` if the operation failed
pub fn open_containing_folder(path: &Path) -> Result<()> {
    if !path.exists() {
        return Err(RustleError::InvalidPath(format!(
            "Path does not exist: {}",
            path.display()
        )));
    }

    log::info!("Opening containing folder for: {}", path.display());

    let explorer = to_wide_string("explorer.exe");
    let verb = to_wide_string("open");
    let args = to_wide_string(&format!("/select,\"{}\"", path.display()));

    let result = unsafe {
        ShellExecuteW(
            HWND::default(),
            PCWSTR(verb.as_ptr()),
            PCWSTR(explorer.as_ptr()),
            PCWSTR(args.as_ptr()),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };

    let result_code = result.0 as isize;

    if result_code > 32 {
        Ok(())
    } else {
        Err(RustleError::LaunchError {
            path: path.to_path_buf(),
            source: std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to open containing folder, code: {}", result_code),
            ),
        })
    }
}

/// Translates ShellExecute error codes to human-readable messages
fn shell_execute_error_message(code: isize) -> &'static str {
    match code {
        0 => "The operating system is out of memory or resources",
        2 => "The specified file was not found",
        3 => "The specified path was not found",
        5 => "Access denied",
        8 => "Not enough memory",
        11 => "Invalid .exe file",
        26 => "A sharing violation occurred",
        27 => "File association is incomplete or invalid",
        28 => "DDE transaction timed out",
        29 => "DDE transaction failed",
        30 => "DDE busy",
        31 => "No application associated with this file type",
        32 => "Dynamic-link library not found",
        _ => "Unknown error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_shell_execute_error_message() {
        assert_eq!(
            shell_execute_error_message(2),
            "The specified file was not found"
        );
        assert_eq!(shell_execute_error_message(5), "Access denied");
        assert_eq!(shell_execute_error_message(999), "Unknown error");
    }

    #[test]
    fn test_launch_nonexistent_file() {
        let result = launch(Path::new(r"C:\nonexistent\file.exe"));
        assert!(result.is_err());
    }

    #[test]
    fn test_open_folder_with_file() {
        let result = open_folder(Path::new(r"C:\Windows\notepad.exe"));
        assert!(result.is_err()); // notepad.exe is not a directory
    }
}
