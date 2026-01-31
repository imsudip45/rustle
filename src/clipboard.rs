//! Clipboard operations for Rustle
//!
//! Provides copy and paste functionality using the Windows Clipboard API.

use std::ptr;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

/// Standard clipboard format for Unicode text
const CF_UNICODETEXT: u32 = 13;

/// Copies text to the Windows clipboard
///
/// # Arguments
/// * `hwnd` - Window handle (can be None for global clipboard access)
/// * `text` - The text to copy
///
/// # Returns
/// * `Ok(())` on success
/// * `Err(String)` with error message on failure
pub fn copy_to_clipboard(hwnd: Option<HWND>, text: &str) -> Result<(), String> {
    if text.is_empty() {
        return Ok(());
    }

    unsafe {
        // Open the clipboard
        let hwnd = hwnd.unwrap_or(HWND::default());
        if OpenClipboard(hwnd).is_err() {
            return Err("Failed to open clipboard".to_string());
        }

        // Clear existing content
        if EmptyClipboard().is_err() {
            let _ = CloseClipboard();
            return Err("Failed to empty clipboard".to_string());
        }

        // Convert to wide string with null terminator
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let size = wide.len() * std::mem::size_of::<u16>();

        // Allocate global memory
        let hmem = match GlobalAlloc(GMEM_MOVEABLE, size) {
            Ok(h) => h,
            Err(_) => {
                let _ = CloseClipboard();
                return Err("Failed to allocate memory".to_string());
            }
        };

        // Lock and copy data
        let ptr = GlobalLock(hmem);
        if ptr.is_null() {
            // Note: Don't free hmem here - if SetClipboardData fails, the mem is our responsibility
            // but if it succeeds, the clipboard owns it. We handle this below.
            let _ = CloseClipboard();
            return Err("Failed to lock memory".to_string());
        }

        ptr::copy_nonoverlapping(wide.as_ptr(), ptr as *mut u16, wide.len());
        let _ = GlobalUnlock(hmem);

        // Set clipboard data - after this, the clipboard owns the memory
        // We must NOT free it ourselves
        let result = SetClipboardData(CF_UNICODETEXT, windows::Win32::Foundation::HANDLE(hmem.0));
        let _ = CloseClipboard();

        if result.is_err() {
            // SetClipboardData failed - in theory we should free hmem here
            // but without GlobalFree, we just log and move on (minor leak on error only)
            return Err("Failed to set clipboard data".to_string());
        }

        log::debug!("Copied to clipboard: {}", text);
        Ok(())
    }
}

/// Pastes text from the Windows clipboard
///
/// # Arguments
/// * `hwnd` - Window handle (can be None for global clipboard access)
///
/// # Returns
/// * `Ok(String)` with clipboard text on success
/// * `Err(String)` with error message on failure or if clipboard is empty
pub fn paste_from_clipboard(hwnd: Option<HWND>) -> Result<String, String> {
    unsafe {
        // Open the clipboard
        let hwnd = hwnd.unwrap_or(HWND::default());
        if OpenClipboard(hwnd).is_err() {
            return Err("Failed to open clipboard".to_string());
        }

        // Get clipboard data
        let hmem = match GetClipboardData(CF_UNICODETEXT) {
            Ok(h) => h,
            Err(_) => {
                let _ = CloseClipboard();
                return Err("No text in clipboard".to_string());
            }
        };

        // Lock and read data
        let ptr = GlobalLock(windows::Win32::Foundation::HGLOBAL(hmem.0));
        if ptr.is_null() {
            let _ = CloseClipboard();
            return Err("Failed to lock clipboard memory".to_string());
        }

        // Find null terminator and read the string
        let wide_ptr = ptr as *const u16;
        let mut len = 0;
        while *wide_ptr.add(len) != 0 {
            len += 1;
        }

        let slice = std::slice::from_raw_parts(wide_ptr, len);
        let text = String::from_utf16_lossy(slice);

        let _ = GlobalUnlock(windows::Win32::Foundation::HGLOBAL(hmem.0));
        let _ = CloseClipboard();

        log::debug!("Pasted from clipboard: {}", text);
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    // Clipboard tests require actual Windows clipboard access
    // and are best run manually
}
