//! Icon extraction for applications
//!
//! This module handles extracting icons from Windows shortcuts (.lnk files)
//! and executables for display in search results.

#![allow(dead_code)]

use std::path::Path;
use windows::Win32::Graphics::Gdi::HDC;
use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
use windows::Win32::UI::Shell::{SHGetFileInfoW, SHGFI_ICON, SHGFI_LARGEICON};
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, DrawIconEx, DI_NORMAL};

/// Icon handle wrapper for safe cleanup
pub struct IconHandle {
    handle: windows::Win32::UI::WindowsAndMessaging::HICON,
}

impl IconHandle {
    pub fn new(handle: windows::Win32::UI::WindowsAndMessaging::HICON) -> Self {
        Self { handle }
    }

    pub fn handle(&self) -> windows::Win32::UI::WindowsAndMessaging::HICON {
        self.handle
    }
}

impl Drop for IconHandle {
    fn drop(&mut self) {
        if !self.handle.is_invalid() {
            unsafe {
                let _ = DestroyIcon(self.handle);
            }
        }
    }
}

/// Extracts an icon from a file path (shortcut or executable)
///
/// Returns None if icon extraction fails.
pub fn extract_icon(path: &Path) -> Option<IconHandle> {
    unsafe {
        let path_wide: Vec<u16> = path
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        // Try to get icon from file using SHGetFileInfoW
        let mut file_info = windows::Win32::UI::Shell::SHFILEINFOW::default();
        let result = SHGetFileInfoW(
            windows::core::PCWSTR(path_wide.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut file_info),
            std::mem::size_of::<windows::Win32::UI::Shell::SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        );

        if result != 0 && !file_info.hIcon.is_invalid() {
            Some(IconHandle::new(file_info.hIcon))
        } else {
            None
        }
    }
}

/// Draws an icon to a device context
pub unsafe fn draw_icon(
    hdc: HDC,
    icon: windows::Win32::UI::WindowsAndMessaging::HICON,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) {
    if !icon.is_invalid() {
        let _ = DrawIconEx(hdc, x, y, icon, width, height, 0, None, DI_NORMAL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_icon_extraction() {
        // Test with a common executable
        let path = Path::new(r"C:\Windows\System32\notepad.exe");
        if path.exists() {
            let icon = extract_icon(path);
            assert!(icon.is_some());
        }
    }
}
