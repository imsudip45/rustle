//! Overlay window and UI rendering for Rustle
//!
//! This module handles creating the floating overlay window,
//! rendering the search UI, and managing window visibility.
//! Uses Win32 API directly for maximum control and performance.

#![allow(dead_code)]

use crate::clipboard::{copy_to_clipboard, paste_from_clipboard};
use crate::config::AppearanceConfig;
use crate::error::{Result, RustleError};
use crate::hotkey::HotkeyManager;
use crate::icons::{draw_icon, extract_icon, IconHandle};
use crate::launcher;
use crate::search::{FlatResult, GroupedResults, ResultType, SearchEngine, SearchResult};
use crate::utils::{to_wide_string, truncate_with_ellipsis};
use std::cell::RefCell;
use std::collections::HashMap;
use std::mem;
use std::path::PathBuf;
use std::sync::Arc;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    DWM_WINDOW_CORNER_PREFERENCE,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontIndirectW, CreatePen,
    CreateRectRgn, CreateSolidBrush, DeleteDC, DeleteObject, DrawTextW, EndPaint, FillRect,
    GetStockObject, GetTextExtentPoint32W, InvalidateRect, RoundRect, SelectClipRgn, SelectObject,
    SetBkMode, SetTextColor, DT_END_ELLIPSIS, DT_LEFT, DT_SINGLELINE, DT_VCENTER, FONT_CHARSET,
    FONT_CLIP_PRECISION, FONT_OUTPUT_PRECISION, FONT_QUALITY, HBRUSH, HFONT, LOGFONTW, NULL_BRUSH,
    PAINTSTRUCT, PS_SOLID, SRCCOPY, TRANSPARENT, GetDC, ReleaseDC,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, SetFocus, VIRTUAL_KEY, VK_A, VK_BACK, VK_C, VK_CONTROL,
    VK_DELETE, VK_DOWN, VK_ESCAPE, VK_LEFT, VK_RETURN, VK_RIGHT, VK_UP, VK_V,
};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetWindowLongPtrW, KillTimer,
    LoadCursorW, PostQuitMessage, RegisterClassExW, SetCursor, SetForegroundWindow,
    SetLayeredWindowAttributes, SetTimer, SetWindowLongPtrW, SetWindowPos, ShowWindow,
    TranslateMessage, CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA, HMENU, HWND_TOPMOST, IDC_ARROW,
    IDC_IBEAM, LWA_ALPHA, MSG, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_HIDE, SW_SHOW, WM_CHAR,
    WM_CLOSE, WM_CREATE, WM_DESTROY, WM_ERASEBKGND, WM_HOTKEY, WM_KEYDOWN, WM_LBUTTONDBLCLK,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_PAINT, WM_TIMER, WNDCLASSEXW,
    WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
    AppendMenuW, CreatePopupMenu, IDI_APPLICATION, LoadIconW, MF_STRING, TPM_BOTTOMALIGN,
    TPM_RIGHTALIGN, TrackPopupMenu, WM_COMMAND, WM_RBUTTONUP, WM_USER,
};

/// Window class name
const CLASS_NAME: &str = "RustleWindowClass";

/// Tray Icon Message ID
const WM_TRAYICON: u32 = WM_USER + 1;
const ID_TRAY_SHOW: usize = 1001;
const ID_TRAY_EXIT: usize = 1002;

/// Timer ID for cursor blinking
const CURSOR_TIMER_ID: usize = 1;

/// Timer ID for debounced search
const SEARCH_TIMER_ID: usize = 2;

/// Cursor blink interval in milliseconds
const CURSOR_BLINK_MS: u32 = 530;

/// Search debounce delay in milliseconds
const SEARCH_DEBOUNCE_MS: u32 = 60;

/// UI dimensions - Modern, spacious layout
const WINDOW_WIDTH: i32 = 800; // Slightly narrower for focused look
const INPUT_HEIGHT: i32 = 56; // Taller input for prominence
const SECTION_HEADER_HEIGHT: i32 = 32;
const ITEM_HEIGHT: i32 = 56; // Taller items for better touch/click
const PADDING: i32 = 16; // More generous padding
const COLUMN_WIDTH: i32 = 250; // Adjusted for 3 columns in 800px
const COLUMN_GAP: i32 = 12; // Larger gap between columns
const ICON_SIZE: i32 = 36; // Slightly larger icons
const ICON_TEXT_GAP: i32 = 14; // Better spacing
const RESULTS_AREA_HEIGHT: i32 = 400; // Compact results area
const CORNER_RADIUS: i32 = 16; // Rounded corners for modern look
const INPUT_CORNER_RADIUS: i32 = 12;
const ITEM_CORNER_RADIUS: i32 = 8;

/// Color scheme for the UI - Premium dark glassmorphism theme
struct Colors {
    // Base colors
    background: u32,
    background_elevated: u32,
    input_bg: u32,

    // Text hierarchy
    text_primary: u32,
    text_secondary: u32,
    text_muted: u32,
    text_accent: u32,

    // Interactive states
    accent: u32,
    accent_hover: u32,
    selection_bg: u32,
    hover_bg: u32,

    // Borders & UI elements
    border: u32,
    border_focused: u32,
    cursor: u32,

    // Semantic colors
    section_text: u32,
    icon_app: u32,
    icon_file: u32,
    icon_folder: u32,

    // Badges/Tags
    badge_bg: u32,
    badge_text: u32,
}

impl Default for Colors {
    fn default() -> Self {
        Self {
            // Premium dark theme inspired by Raycast/Linear
            background: 0xFF0D0D0D,          // Near black, premium feel
            background_elevated: 0xFF1A1A1A, // Slightly elevated panels
            input_bg: 0xFF141414,            // Subtle input background

            // Text with proper contrast hierarchy
            text_primary: 0xFFFFFFFF,   // Pure white for primary
            text_secondary: 0xFFB4B4B4, // Softer secondary
            text_muted: 0xFF6B6B6B,     // Muted for hints
            text_accent: 0xFF3B82F6,    // Accent text (blue)

            // Modern blue accent (similar to Vercel/Linear)
            accent: 0xFF3B82F6,       // Vivid blue
            accent_hover: 0xFF60A5FA, // Lighter on hover
            selection_bg: 0xFF1E3A5F, // Deep blue selection
            hover_bg: 0xFF1F1F1F,     // Subtle hover

            // Subtle borders
            border: 0xFF2A2A2A,         // Barely visible border
            border_focused: 0xFF3B82F6, // Blue border when focused
            cursor: 0xFF3B82F6,         // Cursor matches accent

            // Section headers
            section_text: 0xFF6B6B6B, // Muted section text

            // Icon colors - vibrant but harmonious
            icon_app: 0xFF60A5FA,    // Blue for apps
            icon_file: 0xFF34D399,   // Green for files
            icon_folder: 0xFFFBBF24, // Yellow/gold for folders

            // Badge styling
            badge_bg: 0xFF2A2A2A,
            badge_text: 0xFF6B6B6B,
        }
    }
}

/// Window state
struct WindowState {
    search_engine: Arc<RefCell<SearchEngine>>,
    query: String,
    grouped_results: GroupedResults,
    flat_results: Vec<FlatResult>,
    selected_index: usize,
    hovered_index: Option<usize>, // Currently hovered result index
    visible: bool,
    hotkey_manager: Option<HotkeyManager>,
    appearance: AppearanceConfig,
    font_main: HFONT,
    font_secondary: HFONT,
    font_section: HFONT,
    cursor_visible: bool,
    cursor_position: usize,          // Cursor position in query string
    selection_start: Option<usize>,  // Text selection start
    selection_end: Option<usize>,    // Text selection end
    is_selecting: bool,              // Whether user is dragging to select
    last_click_index: Option<usize>, // For double-click detection
    last_click_time: Option<std::time::Instant>, // For double-click timing
    colors: Colors,
    search_pending: bool,
    hwnd: HWND,
    base_height: i32,                         // Store base window height for reset
    icon_cache: HashMap<PathBuf, IconHandle>, // Cache of extracted icons
    scroll_apps: i32,                         // Scroll offset for Applications column
    scroll_folders: i32,                      // Scroll offset for Folders column
    scroll_files: i32,                        // Scroll offset for Files column
}

impl WindowState {
    fn perform_search(&mut self) {
        self.grouped_results = self.search_engine.borrow().search(&self.query);
        self.flat_results = self.grouped_results.flatten_with_sections();

        // Extract icons for applications
        unsafe {
            self.extract_icons_for_results();
        }

        // Find first selectable item
        self.selected_index = self
            .flat_results
            .iter()
            .position(|r| r.is_selectable())
            .unwrap_or(0);
    }

    fn select_previous(&mut self) {
        if self.flat_results.is_empty() {
            return;
        }

        let mut idx = self.selected_index;
        loop {
            if idx == 0 {
                break;
            }
            idx -= 1;
            if self.flat_results[idx].is_selectable() {
                self.selected_index = idx;
                break;
            }
        }
    }

    fn select_next(&mut self) {
        if self.flat_results.is_empty() {
            return;
        }

        let mut idx = self.selected_index;
        loop {
            if idx >= self.flat_results.len() - 1 {
                break;
            }
            idx += 1;
            if self.flat_results[idx].is_selectable() {
                self.selected_index = idx;
                break;
            }
        }
    }

    fn get_selected_result(&self) -> Option<&SearchResult> {
        if let Some(FlatResult::Item(result)) = self.flat_results.get(self.selected_index) {
            Some(result)
        } else {
            None
        }
    }

    fn launch_selected(&self) -> Result<()> {
        if let Some(result) = self.get_selected_result() {
            launcher::launch(&result.path)?;
        }
        Ok(())
    }

    /// Finds which result item was clicked based on X and Y coordinates (column-aware)
    fn find_clicked_result_index(&self, x: i32, y: i32) -> Option<usize> {
        let results_top = PADDING + INPUT_HEIGHT + 8;
        let column_content_top = results_top + SECTION_HEADER_HEIGHT;

        // Determine which column
        let (result_type, column_x) = if x < PADDING + COLUMN_WIDTH {
            (
                ResultType::Application,
                self.get_column_x(ResultType::Application),
            )
        } else if x < PADDING + COLUMN_WIDTH * 2 + COLUMN_GAP {
            (ResultType::Folder, self.get_column_x(ResultType::Folder))
        } else {
            (ResultType::File, self.get_column_x(ResultType::File))
        };

        if y < column_content_top || y >= column_content_top + RESULTS_AREA_HEIGHT {
            return None;
        }

        // Check if click is within column bounds
        if x < column_x || x >= column_x + COLUMN_WIDTH {
            return None;
        }

        let scroll_offset = self.get_scroll_offset(result_type);
        let relative_y = y - column_content_top + scroll_offset;

        // Find which item in this column
        let results = self.grouped_results.get_by_type(result_type);
        let item_index = (relative_y / ITEM_HEIGHT) as usize;

        if item_index < results.len() {
            // Find the global index in flat_results
            self.flat_results.iter().position(|r| {
                if let FlatResult::Item(r) = r {
                    r.path == results[item_index].path && r.result_type == result_type
                } else {
                    false
                }
            })
        } else {
            None
        }
    }

    /// Launches a result by index
    fn launch_result(&self, index: usize) -> Result<()> {
        if let Some(FlatResult::Item(result)) = self.flat_results.get(index) {
            launcher::launch(&result.path)?;
        }
        Ok(())
    }

    fn clear(&mut self) {
        self.query.clear();
        self.grouped_results = GroupedResults::default();
        self.flat_results.clear();
        self.selected_index = 0;
        self.hovered_index = None;
        self.cursor_position = 0;
        self.selection_start = None;
        self.selection_end = None;
        // Clear icon cache when clearing results
        self.icon_cache.clear();
        // Reset scroll positions
        self.scroll_apps = 0;
        self.scroll_folders = 0;
        self.scroll_files = 0;
    }

    /// Extracts icons for application results
    unsafe fn extract_icons_for_results(&mut self) {
        for flat_result in &self.flat_results {
            if let FlatResult::Item(result) = flat_result {
                // Only extract icons for applications
                if result.result_type == ResultType::Application {
                    // Check if icon is already cached
                    if !self.icon_cache.contains_key(&result.path) {
                        if let Some(icon) = extract_icon(&result.path) {
                            self.icon_cache.insert(result.path.clone(), icon);
                        }
                    }
                }
            }
        }
    }

    fn get_selection_range(&self) -> (usize, usize) {
        if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
            if start < end {
                (start, end)
            } else {
                (end, start)
            }
        } else {
            (0, 0)
        }
    }

    fn has_selection(&self) -> bool {
        self.selection_start.is_some() && self.selection_end.is_some()
    }

    fn delete_selection(&mut self) {
        if self.has_selection() {
            let (start, end) = self.get_selection_range();
            self.query.drain(start..end);
            self.cursor_position = start;
            self.selection_start = None;
            self.selection_end = None;
        }
    }

    fn select_all(&mut self) {
        if !self.query.is_empty() {
            self.selection_start = Some(0);
            self.selection_end = Some(self.query.len());
        }
    }

    fn copy_selection(&self) -> Option<String> {
        if self.has_selection() {
            let (start, end) = self.get_selection_range();
            Some(self.query[start..end].to_string())
        } else {
            None
        }
    }

    fn calculate_height(&self) -> i32 {
        let base = INPUT_HEIGHT + PADDING * 2;
        // Fixed height for column-based layout
        base + SECTION_HEADER_HEIGHT + RESULTS_AREA_HEIGHT + PADDING
    }

    /// Gets scroll offset for a specific result type
    fn get_scroll_offset(&self, result_type: ResultType) -> i32 {
        match result_type {
            ResultType::Application => self.scroll_apps,
            ResultType::Folder => self.scroll_folders,
            ResultType::File => self.scroll_files,
        }
    }

    /// Sets scroll offset for a specific result type
    fn set_scroll_offset(&mut self, result_type: ResultType, offset: i32) {
        match result_type {
            ResultType::Application => self.scroll_apps = offset.max(0),
            ResultType::Folder => self.scroll_folders = offset.max(0),
            ResultType::File => self.scroll_files = offset.max(0),
        }
    }

    /// Gets column X position for a result type
    fn get_column_x(&self, result_type: ResultType) -> i32 {
        match result_type {
            ResultType::Application => PADDING,
            ResultType::Folder => PADDING + COLUMN_WIDTH + COLUMN_GAP,
            ResultType::File => PADDING + (COLUMN_WIDTH + COLUMN_GAP) * 2,
        }
    }
}

/// Creates and runs the main application window
pub fn create_and_run(search_engine: SearchEngine, appearance: AppearanceConfig) -> Result<()> {
    let class_name = to_wide_string(CLASS_NAME);

    unsafe {
        let hinstance = GetModuleHandleW(PCWSTR::null()).map_err(|e| {
            RustleError::window_creation(format!("GetModuleHandle failed: {:?}", e))
        })?;

        let wc = WNDCLASSEXW {
            cbSize: mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance.into(),
            hIcon: Default::default(),
            hCursor: LoadCursorW(None, IDC_IBEAM)
                .map_err(|e| RustleError::window_creation(format!("LoadCursor failed: {:?}", e)))?,
            hbrBackground: HBRUSH(GetStockObject(NULL_BRUSH).0),
            lpszMenuName: PCWSTR::null(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            hIconSm: Default::default(),
        };

        let atom = RegisterClassExW(&wc);
        if atom == 0 {
            return Err(RustleError::WindowClassRegistration(
                "Failed to register window class".to_string(),
            ));
        }

        let screen_width = windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(
            windows::Win32::UI::WindowsAndMessaging::SM_CXSCREEN,
        );
        let screen_height = windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(
            windows::Win32::UI::WindowsAndMessaging::SM_CYSCREEN,
        );

        let window_height = INPUT_HEIGHT + PADDING * 2;
        let x = (screen_width - WINDOW_WIDTH) / 2;
        let y = screen_height / 5;

        let title = to_wide_string("Rustle");

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_POPUP,
            x,
            y,
            WINDOW_WIDTH,
            window_height,
            HWND::default(),
            HMENU::default(),
            hinstance,
            None,
        )
        .map_err(|e| RustleError::window_creation(format!("CreateWindowEx failed: {:?}", e)))?;

        // Set to fully opaque (255 = no transparency)
        SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA).map_err(|e| {
            RustleError::window_creation(format!("SetLayeredWindowAttributes failed: {:?}", e))
        })?;

        // Enable rounded corners on Windows 11
        let corner_preference = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner_preference as *const _ as *const _,
            mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        );

        // Note: Not using DwmExtendFrameIntoClientArea to avoid transparency issues

        let font_main = create_font("Segoe UI", 16, 400);
        let font_secondary = create_font("Segoe UI", 12, 400);
        let font_section = create_font("Segoe UI", 11, 600);

        let base_height = INPUT_HEIGHT + PADDING * 2;

        let state = Box::new(WindowState {
            search_engine: Arc::new(RefCell::new(search_engine)),
            query: String::new(),
            grouped_results: GroupedResults::default(),
            flat_results: Vec::new(),
            selected_index: 0,
            hovered_index: None,
            visible: false,
            hotkey_manager: None,
            appearance,
            font_main,
            font_secondary,
            font_section,
            cursor_visible: true,
            cursor_position: 0,
            selection_start: None,
            selection_end: None,
            is_selecting: false,
            last_click_index: None,
            last_click_time: None,
            colors: Colors::default(),
            search_pending: false,
            hwnd,
            base_height,
            icon_cache: HashMap::new(),
            scroll_apps: 0,
            scroll_folders: 0,
            scroll_files: 0,
        });

        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);

        let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
        if !state_ptr.is_null() {
            let state = &mut *state_ptr;
            let mut hotkey_manager = HotkeyManager::new(hwnd);
            if let Err(e) = hotkey_manager.register_default() {
                log::error!("Failed to register hotkey: {}", e);
            }
            state.hotkey_manager = Some(hotkey_manager);
            
            // Initialize tray icon and show window
            init_tray_icon(hwnd);
            show_window(hwnd, state);
        }

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, HWND::default(), 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        Ok(())
    }
}

fn create_font(face: &str, height: i32, weight: i32) -> HFONT {
    let mut lf = LOGFONTW::default();
    lf.lfHeight = -height;
    lf.lfWeight = weight;
    lf.lfCharSet = FONT_CHARSET(1);
    lf.lfQuality = FONT_QUALITY(5);
    lf.lfOutPrecision = FONT_OUTPUT_PRECISION(3);
    lf.lfClipPrecision = FONT_CLIP_PRECISION(2);

    for (i, c) in face.encode_utf16().enumerate() {
        if i < lf.lfFaceName.len() - 1 {
            lf.lfFaceName[i] = c;
        }
    }

    unsafe { CreateFontIndirectW(&lf) }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            log::debug!("Window created");
            LRESULT(0)
        }

        WM_TRAYICON => {
            if lparam.0 == WM_RBUTTONUP as isize || lparam.0 == WM_LBUTTONUP as isize {
                // Show context menu
                let hmenu = CreatePopupMenu().unwrap_or_default();
                let _ = AppendMenuW(hmenu, MF_STRING, ID_TRAY_SHOW, PCWSTR(to_wide_string("Open").as_ptr()));
                let _ = AppendMenuW(hmenu, MF_STRING, ID_TRAY_EXIT, PCWSTR(to_wide_string("Exit").as_ptr()));
                
                let mut pt = windows::Win32::Foundation::POINT::default();
                let _ = windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut pt);
                let _ = SetForegroundWindow(hwnd);
                
                let _ = TrackPopupMenu(
                    hmenu,
                    TPM_BOTTOMALIGN | TPM_RIGHTALIGN,
                    pt.x,
                    pt.y,
                    0,
                    hwnd,
                    None
                );
            }
            LRESULT(0)
        }

        WM_COMMAND => {
            let id = wparam.0 & 0xFFFF;
            match id {
                ID_TRAY_SHOW => {
                    let state = get_window_state(hwnd);
                    if let Some(state) = state {
                        show_window(hwnd, state);
                    }
                }
                ID_TRAY_EXIT => {
                    PostQuitMessage(0);
                }
                _ => {}
            }
            LRESULT(0)
        }

        WM_ERASEBKGND => LRESULT(1),

        WM_DESTROY => {
            remove_tray_icon(hwnd);
            let _ = KillTimer(hwnd, CURSOR_TIMER_ID);
            let _ = KillTimer(hwnd, SEARCH_TIMER_ID);

            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if !state_ptr.is_null() {
                let state = Box::from_raw(state_ptr);
                if !state.font_main.is_invalid() {
                    let _ = DeleteObject(state.font_main);
                }
                if !state.font_secondary.is_invalid() {
                    let _ = DeleteObject(state.font_secondary);
                }
                if !state.font_section.is_invalid() {
                    let _ = DeleteObject(state.font_section);
                }
            }
            PostQuitMessage(0);
            LRESULT(0)
        }

        WM_CLOSE => {
            let state = get_window_state(hwnd);
            if let Some(state) = state {
                hide_window(hwnd, state);
            }
            LRESULT(0)
        }

        WM_TIMER => {
            let state = get_window_state(hwnd);
            if let Some(state) = state {
                if wparam.0 == CURSOR_TIMER_ID {
                    state.cursor_visible = !state.cursor_visible;
                    let _ = InvalidateRect(hwnd, None, false);
                } else if wparam.0 == SEARCH_TIMER_ID {
                    let _ = KillTimer(hwnd, SEARCH_TIMER_ID);
                    state.search_pending = false;
                    state.perform_search();
                    update_window_size(hwnd, state);
                    let _ = InvalidateRect(hwnd, None, false);
                }
            }
            LRESULT(0)
        }

        WM_HOTKEY => {
            log::debug!("Hotkey pressed");
            let state = get_window_state(hwnd);
            if let Some(state) = state {
                if state.visible {
                    hide_window(hwnd, state);
                } else {
                    show_window(hwnd, state);
                }
            }
            LRESULT(0)
        }

        WM_KEYDOWN => {
            let vk = wparam.0 as u16;
            let state = get_window_state(hwnd);

            if let Some(state) = state {
                state.cursor_visible = true;

                // Check for Ctrl key
                let ctrl_pressed =
                    unsafe { (GetKeyState(VK_CONTROL.0 as i32) as u16) & 0x8000 != 0 };

                match VIRTUAL_KEY(vk) {
                    VK_ESCAPE => {
                        hide_window(hwnd, state);
                    }
                    VK_UP => {
                        state.select_previous();
                        let _ = InvalidateRect(hwnd, None, false);
                    }
                    VK_DOWN => {
                        state.select_next();
                        let _ = InvalidateRect(hwnd, None, false);
                    }
                    VK_RETURN => {
                        if let Err(e) = state.launch_selected() {
                            log::error!("Failed to launch: {}", e);
                        } else {
                            hide_window(hwnd, state);
                        }
                    }
                    VK_LEFT => {
                        if ctrl_pressed {
                            // Ctrl+Left: Move to word start
                            // Simple implementation - just move to start
                            state.cursor_position = 0;
                        } else if state.cursor_position > 0 {
                            state.cursor_position -= 1;
                        }
                        state.selection_start = None;
                        state.selection_end = None;
                        let _ = InvalidateRect(hwnd, None, false);
                    }
                    VK_RIGHT => {
                        if ctrl_pressed {
                            // Ctrl+Right: Move to word end
                            state.cursor_position = state.query.len();
                        } else if state.cursor_position < state.query.len() {
                            state.cursor_position += 1;
                        }
                        state.selection_start = None;
                        state.selection_end = None;
                        let _ = InvalidateRect(hwnd, None, false);
                    }
                    VK_BACK => {
                        if state.has_selection() {
                            state.delete_selection();
                        } else if state.cursor_position > 0 {
                            state.cursor_position -= 1;
                            state.query.remove(state.cursor_position);
                        }
                        schedule_search(hwnd, state);
                        let _ = InvalidateRect(hwnd, None, false);
                    }
                    VK_DELETE => {
                        if state.has_selection() {
                            state.delete_selection();
                        } else if state.cursor_position < state.query.len() {
                            state.query.remove(state.cursor_position);
                        }
                        schedule_search(hwnd, state);
                        let _ = InvalidateRect(hwnd, None, false);
                    }
                    VK_A => {
                        if ctrl_pressed {
                            // Ctrl+A: Select all
                            state.select_all();
                            let _ = InvalidateRect(hwnd, None, false);
                        }
                    }
                    VK_C => {
                        if ctrl_pressed {
                            // Ctrl+C: Copy to clipboard
                            if let Some(text) = state.copy_selection() {
                                if let Err(e) = copy_to_clipboard(Some(hwnd), &text) {
                                    log::error!("Failed to copy: {}", e);
                                }
                            }
                        }
                    }
                    VK_V => {
                        if ctrl_pressed {
                            // Ctrl+V: Paste from clipboard
                            if let Ok(text) = paste_from_clipboard(Some(hwnd)) {
                                // Delete selection if any
                                if state.has_selection() {
                                    state.delete_selection();
                                }
                                // Insert pasted text at cursor
                                for c in text.chars() {
                                    if c != '\r' && c != '\n' {
                                        state.query.insert(state.cursor_position, c);
                                        state.cursor_position += 1;
                                    }
                                }
                                schedule_search(hwnd, state);
                                let _ = InvalidateRect(hwnd, None, false);
                            }
                        }
                    }
                    _ => {}
                }
            }
            LRESULT(0)
        }

        WM_CHAR => {
            let c = char::from_u32(wparam.0 as u32);
            let state = get_window_state(hwnd);

            if let (Some(c), Some(state)) = (c, state) {
                state.cursor_visible = true;

                // Check for Ctrl key (Ctrl+V for paste, etc.)
                let ctrl_pressed =
                    unsafe { (GetKeyState(VK_CONTROL.0 as i32) as u16) & 0x8000 != 0 };

                if ctrl_pressed {
                    // Ctrl key combinations handled in WM_KEYDOWN
                    // Skip character insertion for Ctrl+key combinations
                } else if c.is_alphanumeric() || c.is_whitespace() || c.is_ascii_punctuation() {
                    if c != '\r' && c != '\n' && c != '\x08' {
                        // Delete selection if any
                        if state.has_selection() {
                            state.delete_selection();
                        }

                        // Insert character at cursor
                        state.query.insert(state.cursor_position, c);
                        state.cursor_position += 1;
                        schedule_search(hwnd, state);
                        let _ = InvalidateRect(hwnd, None, false);
                    }
                }
            }
            LRESULT(0)
        }

        WM_LBUTTONDOWN => {
            let state = get_window_state(hwnd);
            if let Some(state) = state {
                let x = (lparam.0 & 0xFFFF) as i32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i32;

                // Check if click is in input area
                let input_rect = RECT {
                    left: PADDING,
                    top: PADDING,
                    right: WINDOW_WIDTH - PADDING,
                    bottom: PADDING + INPUT_HEIGHT,
                };

                if x >= input_rect.left
                    && x < input_rect.right
                    && y >= input_rect.top
                    && y < input_rect.bottom
                {
                    // Click in input area - handle text selection
                    state.is_selecting = true;
                    // Calculate cursor position from click
                    let text_offset = PADDING + 48; // Icon (16+28=44) + padding
                    let target_x = x - text_offset;
                    let cursor_idx = calculate_cursor_from_x(hwnd, &state.query, target_x, state.font_main);
                    
                    state.cursor_position = cursor_idx;
                    state.selection_start = Some(cursor_idx);
                    state.selection_end = Some(cursor_idx);
                    
                    let _ = InvalidateRect(hwnd, None, false);
                    let _ = SetCapture(hwnd);
                } else {
                    // Click might be on a result item
                    let x = (lparam.0 & 0xFFFF) as i32;
                    if let Some(clicked_index) = state.find_clicked_result_index(x, y) {
                        // Update selection to clicked item with visual feedback
                        state.selected_index = clicked_index;
                        state.hovered_index = Some(clicked_index);
                        // Brief visual feedback - repaint immediately
                        let _ = InvalidateRect(hwnd, None, false);
                    }
                }
            }
            LRESULT(0)
        }

        WM_MOUSEMOVE => {
            let state = get_window_state(hwnd);
            if let Some(state) = state {
                let x = (lparam.0 & 0xFFFF) as i32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i32;

                // Handle active selection regardless of where mouse is (dragging)
                if state.is_selecting {
                     let text_offset = PADDING + 48;
                     let target_x = x - text_offset;
                     let cursor_idx = calculate_cursor_from_x(hwnd, &state.query, target_x, state.font_main);
                     
                     state.cursor_position = cursor_idx;
                     state.selection_end = Some(cursor_idx);
                     let _ = InvalidateRect(hwnd, None, false);
                     
                     // Ensure cursor is I-Beam during selection
                     let _ = SetCursor(LoadCursorW(None, IDC_IBEAM).unwrap_or_default());
                     return LRESULT(0);
                }

                // Check if mouse is outside window bounds
                let mut rect = RECT::default();
                let _ = windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut rect);
                let mouse_outside = x < 0 || x >= rect.right || y < 0 || y >= rect.bottom;

                if mouse_outside {
                    // Mouse left window - clear hover
                    if state.hovered_index.is_some() {
                        state.hovered_index = None;
                        let _ = InvalidateRect(hwnd, None, false);
                    }
                    return LRESULT(0);
                }

                // Check if mouse is over input area or results
                let input_rect = RECT {
                    left: PADDING,
                    top: PADDING,
                    right: WINDOW_WIDTH - PADDING,
                    bottom: PADDING + INPUT_HEIGHT,
                };

                let is_over_input = x >= input_rect.left
                    && x < input_rect.right
                    && y >= input_rect.top
                    && y < input_rect.bottom;

                if is_over_input {
                    // Over input area - use text cursor
                    let _ = SetCursor(LoadCursorW(None, IDC_IBEAM).unwrap_or_default());
                } else {
                    // Over results area - use arrow cursor
                    let _ = SetCursor(LoadCursorW(None, IDC_ARROW).unwrap_or_default());

                    // Track hover over result items
                    let new_hovered = state.find_clicked_result_index(x, y);
                    if new_hovered != state.hovered_index {
                        state.hovered_index = new_hovered;
                        let _ = InvalidateRect(hwnd, None, false);
                    }
                }
            }
            LRESULT(0)
        }

        WM_LBUTTONUP => {
            let state = get_window_state(hwnd);
            if let Some(state) = state {
                let x = (lparam.0 & 0xFFFF) as i32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i32;

                if state.is_selecting {
                    // Was selecting text - stop selection
                    state.is_selecting = false;
                    let _ = ReleaseCapture();
                } else {
                    // Check if click is on a result item
                    if let Some(clicked_index) = state.find_clicked_result_index(x, y) {
                        // Check for double-click
                        let now = std::time::Instant::now();
                        let is_double_click = state.last_click_index == Some(clicked_index)
                            && state
                                .last_click_time
                                .map_or(false, |t| now.duration_since(t).as_millis() < 500);

                        if is_double_click {
                            // Double-click - launch it
                            if let Err(e) = state.launch_result(clicked_index) {
                                log::error!("Failed to launch: {}", e);
                            } else {
                                hide_window(hwnd, state);
                            }
                            // Reset double-click tracking
                            state.last_click_index = None;
                            state.last_click_time = None;
                        } else {
                            // Single click - just select it
                            state.selected_index = clicked_index;
                            state.hovered_index = Some(clicked_index);
                            state.last_click_index = Some(clicked_index);
                            state.last_click_time = Some(now);
                            let _ = InvalidateRect(hwnd, None, false);
                        }
                    }
                }
            }
            LRESULT(0)
        }

        WM_LBUTTONDBLCLK => {
            // Handle explicit double-click message
            let state = get_window_state(hwnd);
            if let Some(state) = state {
                let x = (lparam.0 & 0xFFFF) as i32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i32;
                if let Some(clicked_index) = state.find_clicked_result_index(x, y) {
                    if let Err(e) = state.launch_result(clicked_index) {
                        log::error!("Failed to launch: {}", e);
                    } else {
                        hide_window(hwnd, state);
                    }
                }
            }
            LRESULT(0)
        }

        WM_MOUSEWHEEL => {
            let state = get_window_state(hwnd);
            if let Some(state) = state {
                let x = (lparam.0 & 0xFFFF) as i32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i32;
                let delta = (wparam.0 >> 16) as i16 as i32; // Wheel delta

                // Determine which column the mouse is over
                let results_top = PADDING + INPUT_HEIGHT + 8;
                let column_content_top = results_top + SECTION_HEADER_HEIGHT;

                if y >= column_content_top && y < column_content_top + RESULTS_AREA_HEIGHT {
                    let result_type = if x < PADDING + COLUMN_WIDTH {
                        ResultType::Application
                    } else if x < PADDING + COLUMN_WIDTH * 2 + COLUMN_GAP {
                        ResultType::Folder
                    } else {
                        ResultType::File
                    };

                    // Scroll the column (negative delta = scroll up, positive = scroll down)
                    let scroll_delta = -delta / 40; // Convert wheel units to pixels
                    let current_scroll = state.get_scroll_offset(result_type);
                    let max_scroll = {
                        let results = state.grouped_results.get_by_type(result_type);
                        let total_height = results.len() as i32 * ITEM_HEIGHT;
                        (total_height - RESULTS_AREA_HEIGHT).max(0)
                    };
                    let new_scroll = (current_scroll + scroll_delta).max(0).min(max_scroll);
                    state.set_scroll_offset(result_type, new_scroll);
                    let _ = InvalidateRect(hwnd, None, false);
                }
            }
            LRESULT(0)
        }

        WM_PAINT => {
            let state = get_window_state(hwnd);
            if let Some(state) = state {
                paint_window(hwnd, state);
            }
            LRESULT(0)
        }

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn show_window(hwnd: HWND, state: &mut WindowState) {
    state.visible = true;
    state.cursor_visible = true;

    let _ = SetTimer(hwnd, CURSOR_TIMER_ID, CURSOR_BLINK_MS, None);

    // Show and activate window properly
    let _ = ShowWindow(hwnd, SW_SHOW);
    let _ = SetWindowPos(
        hwnd,
        HWND_TOPMOST,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
    );

    // Force focus - this is critical for keyboard input
    let _ = SetForegroundWindow(hwnd);
    let _ = SetFocus(hwnd);

    let _ = InvalidateRect(hwnd, None, false);
}

unsafe fn hide_window(hwnd: HWND, state: &mut WindowState) {
    state.visible = false;
    state.clear();
    state.search_pending = false;

    let _ = KillTimer(hwnd, CURSOR_TIMER_ID);
    let _ = KillTimer(hwnd, SEARCH_TIMER_ID);

    // Reset window to base height
    let _ = SetWindowPos(
        hwnd,
        HWND_TOPMOST,
        0,
        0,
        WINDOW_WIDTH,
        state.base_height,
        SWP_NOMOVE,
    );

    let _ = ShowWindow(hwnd, SW_HIDE);
}

unsafe fn schedule_search(hwnd: HWND, state: &mut WindowState) {
    if state.search_pending {
        let _ = KillTimer(hwnd, SEARCH_TIMER_ID);
    }

    if state.query.is_empty() {
        state.grouped_results = GroupedResults::default();
        state.flat_results.clear();
        state.selected_index = 0;
        update_window_size(hwnd, state);
        return;
    }

    state.search_pending = true;
    let _ = SetTimer(hwnd, SEARCH_TIMER_ID, SEARCH_DEBOUNCE_MS, None);
}

unsafe fn get_window_state(hwnd: HWND) -> Option<&'static mut WindowState> {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
    if ptr.is_null() {
        None
    } else {
        Some(&mut *ptr)
    }
}

unsafe fn update_window_size(hwnd: HWND, state: &WindowState) {
    let new_height = state.calculate_height();
    let _ = SetWindowPos(
        hwnd,
        HWND_TOPMOST,
        0,
        0,
        WINDOW_WIDTH,
        new_height,
        SWP_NOMOVE,
    );
}

unsafe fn paint_window(hwnd: HWND, state: &WindowState) {
    let mut ps = PAINTSTRUCT::default();
    let hdc_screen = BeginPaint(hwnd, &mut ps);

    let colors = &state.colors;

    let mut rect = RECT::default();
    let _ = windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut rect);

    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;

    // Double buffering
    let hdc_buffer = CreateCompatibleDC(hdc_screen);
    let hbm_buffer = CreateCompatibleBitmap(hdc_screen, width, height);
    let hbm_old = SelectObject(hdc_buffer, hbm_buffer);
    let hdc = hdc_buffer;

    // Brushes
    let bg_brush = CreateSolidBrush(COLORREF(colors.background & 0x00FFFFFF));
    let input_bg_brush = CreateSolidBrush(COLORREF(colors.input_bg & 0x00FFFFFF));
    let selection_brush = CreateSolidBrush(COLORREF(colors.selection_bg & 0x00FFFFFF));
    let hover_brush = CreateSolidBrush(COLORREF(colors.hover_bg & 0x00FFFFFF));
    let accent_brush = CreateSolidBrush(COLORREF(colors.accent & 0x00FFFFFF));

    // Background
    FillRect(hdc, &rect, bg_brush);
    SetBkMode(hdc, TRANSPARENT);

    // Input area
    let input_rect = RECT {
        left: PADDING,
        top: PADDING,
        right: rect.right - PADDING,
        bottom: PADDING + INPUT_HEIGHT,
    };

    let input_pen = CreatePen(PS_SOLID, 1, COLORREF(colors.border & 0x00FFFFFF));
    let old_pen = SelectObject(hdc, input_pen);
    let old_brush = SelectObject(hdc, input_bg_brush);
    let _ = RoundRect(
        hdc,
        input_rect.left,
        input_rect.top,
        input_rect.right,
        input_rect.bottom,
        INPUT_CORNER_RADIUS,
        INPUT_CORNER_RADIUS,
    );
    SelectObject(hdc, old_brush);
    SelectObject(hdc, old_pen);
    let _ = DeleteObject(input_pen);

    let _ = DeleteObject(input_pen);

    // Hint text (Esc to close)
    SelectObject(hdc, state.font_secondary);
    SetTextColor(hdc, COLORREF(colors.text_muted & 0x00FFFFFF));
    let mut hint_rect = input_rect;
    hint_rect.right -= 16; // Padding from right
    DrawTextW(
        hdc,
        &mut to_wide_chars("Esc to close"),
        &mut hint_rect,
        windows::Win32::Graphics::Gdi::DT_RIGHT | DT_SINGLELINE | DT_VCENTER,
    );

    // Search icon
    SelectObject(hdc, state.font_main);
    SetTextColor(hdc, COLORREF(colors.text_muted & 0x00FFFFFF));
    let icon_rect = RECT {
        left: input_rect.left + 16,
        top: input_rect.top,
        right: input_rect.left + 44,
        bottom: input_rect.bottom,
    };
    let mut icon_rect_mut = icon_rect;
    DrawTextW(
        hdc,
        &mut to_wide_chars("🔍"),
        &mut icon_rect_mut,
        DT_LEFT | DT_SINGLELINE | DT_VCENTER,
    );

    // Query or placeholder
    let text_left = input_rect.left + 48;
    let text_rect = RECT {
        left: text_left,
        top: input_rect.top,
        right: input_rect.right - 16,
        bottom: input_rect.bottom,
    };

    if state.query.is_empty() {
        SetTextColor(hdc, COLORREF(colors.text_muted & 0x00FFFFFF));
        let mut ph_rect = text_rect;
        DrawTextW(
            hdc,
            &mut to_wide_chars("Search applications, files, and folders..."),
            &mut ph_rect,
            DT_LEFT | DT_SINGLELINE | DT_VCENTER,
        );
    } else {
        // Draw text with selection highlight
        SelectObject(hdc, state.font_main);

        // Draw selection background if any
        if state.has_selection() {
            let (sel_start, sel_end) = state.get_selection_range();
            if sel_start < state.query.len() && sel_end <= state.query.len() {
                // Calculate positions
                let before_sel = &state.query[..sel_start];
                let selected = &state.query[sel_start..sel_end];

                let mut before_size = windows::Win32::Foundation::SIZE::default();
                let before_wide = to_wide_chars(before_sel);
                if !before_wide.is_empty() {
                    let _ = GetTextExtentPoint32W(hdc, &before_wide, &mut before_size);
                }

                let mut sel_size = windows::Win32::Foundation::SIZE::default();
                let sel_wide = to_wide_chars(selected);
                if !sel_wide.is_empty() {
                    let _ = GetTextExtentPoint32W(hdc, &sel_wide, &mut sel_size);
                }

                // Draw selection background
                let sel_rect = RECT {
                    left: text_left + before_size.cx,
                    top: input_rect.top + 4,
                    right: text_left + before_size.cx + sel_size.cx,
                    bottom: input_rect.bottom - 4,
                };
                FillRect(hdc, &sel_rect, accent_brush);
            }
        }

        // Draw text
        SetTextColor(hdc, COLORREF(colors.text_primary & 0x00FFFFFF));
        let mut query_rect = text_rect;
        DrawTextW(
            hdc,
            &mut to_wide_chars(&state.query),
            &mut query_rect,
            DT_LEFT | DT_SINGLELINE | DT_VCENTER,
        );
    }

    // Blinking cursor at cursor_position
    if state.cursor_visible && !state.query.is_empty() {
        let cursor_text = &state.query[..state.cursor_position.min(state.query.len())];
        let cursor_wide = to_wide_chars(cursor_text);
        let mut text_size = windows::Win32::Foundation::SIZE::default();
        if !cursor_wide.is_empty() {
            let _ = GetTextExtentPoint32W(hdc, &cursor_wide, &mut text_size);
        }

        let cursor_x = text_left + text_size.cx;
        let cursor_top = input_rect.top + 14;
        let cursor_bottom = input_rect.bottom - 14;

        let cursor_pen = CreatePen(PS_SOLID, 2, COLORREF(colors.cursor & 0x00FFFFFF));
        SelectObject(hdc, cursor_pen);
        let _ = windows::Win32::Graphics::Gdi::MoveToEx(hdc, cursor_x, cursor_top, None);
        let _ = windows::Win32::Graphics::Gdi::LineTo(hdc, cursor_x, cursor_bottom);
        let _ = DeleteObject(cursor_pen);
    } else if state.cursor_visible && state.query.is_empty() {
        // Cursor at start when empty
        let cursor_x = text_left;
        let cursor_top = input_rect.top + 14;
        let cursor_bottom = input_rect.bottom - 14;

        let cursor_pen = CreatePen(PS_SOLID, 2, COLORREF(colors.cursor & 0x00FFFFFF));
        SelectObject(hdc, cursor_pen);
        let _ = windows::Win32::Graphics::Gdi::MoveToEx(hdc, cursor_x, cursor_top, None);
        let _ = windows::Win32::Graphics::Gdi::LineTo(hdc, cursor_x, cursor_bottom);
        let _ = DeleteObject(cursor_pen);
    }

    // Column-based results layout
    let results_top = input_rect.bottom + 8;

    if !state.flat_results.is_empty() {
        // Render each column
        let column_types = [
            ResultType::Application,
            ResultType::Folder,
            ResultType::File,
        ];
        for result_type in &column_types {
            let column_x = state.get_column_x(*result_type);
            let scroll_offset = state.get_scroll_offset(*result_type);

            // Column header
            SelectObject(hdc, state.font_section);
            SetTextColor(hdc, COLORREF(colors.section_text & 0x00FFFFFF));
            let header_rect = RECT {
                left: column_x,
                top: results_top,
                right: column_x + COLUMN_WIDTH,
                bottom: results_top + SECTION_HEADER_HEIGHT,
            };
            let mut header_rect_mut = header_rect;
            DrawTextW(
                hdc,
                &mut to_wide_chars(result_type.section_header()),
                &mut header_rect_mut,
                DT_LEFT | DT_SINGLELINE | DT_VCENTER,
            );

            // Column content area (with clipping)
            let column_content_top = results_top + SECTION_HEADER_HEIGHT;
            let column_clip = RECT {
                left: column_x,
                top: column_content_top,
                right: column_x + COLUMN_WIDTH,
                bottom: column_content_top + RESULTS_AREA_HEIGHT,
            };

            // Set clipping region for this column
            let clip_region = CreateRectRgn(
                column_clip.left,
                column_clip.top,
                column_clip.right,
                column_clip.bottom,
            );
            let _ = SelectClipRgn(hdc, clip_region);

            // Get results for this column
            let column_results: Vec<_> = state
                .grouped_results
                .get_by_type(*result_type)
                .iter()
                .collect();

            let mut y = column_content_top - scroll_offset;
            for (_idx, result) in column_results.iter().enumerate() {
                let item_rect = RECT {
                    left: column_x + 8,
                    top: y,
                    right: column_x + COLUMN_WIDTH - 8,
                    bottom: y + ITEM_HEIGHT,
                };

                // Only draw if visible in clip region
                if item_rect.bottom >= column_clip.top && item_rect.top <= column_clip.bottom {
                    // Calculate global index for hover/selection
                    let global_idx = state
                        .flat_results
                        .iter()
                        .position(|r| {
                            if let FlatResult::Item(r) = r {
                                r.path == result.path && r.result_type == *result_type
                            } else {
                                false
                            }
                        })
                        .unwrap_or(0);

                    // Hover highlight
                    let is_hovered = state.hovered_index == Some(global_idx)
                        && global_idx != state.selected_index;
                    if is_hovered {
                        let hover_pen =
                            CreatePen(PS_SOLID, 0, COLORREF(colors.hover_bg & 0x00FFFFFF));
                        let old_hover_pen = SelectObject(hdc, hover_pen);
                        let old_hover_brush = SelectObject(hdc, hover_brush);
                        let _ = RoundRect(
                            hdc,
                            item_rect.left,
                            item_rect.top,
                            item_rect.right,
                            item_rect.bottom,
                            ITEM_CORNER_RADIUS,
                            ITEM_CORNER_RADIUS,
                        );
                        SelectObject(hdc, old_hover_brush);
                        SelectObject(hdc, old_hover_pen);
                        let _ = DeleteObject(hover_pen);
                    }

                    // Selection highlight
                    if global_idx == state.selected_index {
                        let sel_pen =
                            CreatePen(PS_SOLID, 0, COLORREF(colors.selection_bg & 0x00FFFFFF));
                        let old_sel_pen = SelectObject(hdc, sel_pen);
                        let old_sel_brush = SelectObject(hdc, selection_brush);
                        let _ = RoundRect(
                            hdc,
                            item_rect.left,
                            item_rect.top,
                            item_rect.right,
                            item_rect.bottom,
                            ITEM_CORNER_RADIUS,
                            ITEM_CORNER_RADIUS,
                        );
                        SelectObject(hdc, old_sel_brush);
                        SelectObject(hdc, old_sel_pen);
                        let _ = DeleteObject(sel_pen);

                        // Accent bar
                        let accent_bar = RECT {
                            left: item_rect.left + 4,
                            top: item_rect.top + 10,
                            right: item_rect.left + 7,
                            bottom: item_rect.bottom - 10,
                        };
                        FillRect(hdc, &accent_bar, accent_brush);
                    }

                    // Icon area (adjusted spacing)
                    let icon_x = item_rect.left + 8;
                    let icon_item_rect = RECT {
                        left: icon_x,
                        top: item_rect.top + 10,
                        right: icon_x + ICON_SIZE,
                        bottom: item_rect.bottom - 10,
                    };

                    // Draw icon
                    match result.result_type {
                        ResultType::Application => {
                            if let Some(icon_handle) = state.icon_cache.get(&result.path) {
                                draw_icon(
                                    hdc,
                                    icon_handle.handle(),
                                    icon_item_rect.left,
                                    icon_item_rect.top,
                                    ICON_SIZE,
                                    ICON_SIZE,
                                );
                            } else {
                                let icon_color = colors.icon_app;
                                SelectObject(hdc, state.font_main);
                                SetTextColor(hdc, COLORREF(icon_color & 0x00FFFFFF));
                                let mut icon_item_rect_mut = icon_item_rect;
                                DrawTextW(
                                    hdc,
                                    &mut to_wide_chars("⚡"),
                                    &mut icon_item_rect_mut,
                                    DT_LEFT | DT_SINGLELINE | DT_VCENTER,
                                );
                            }
                        }
                        ResultType::File => {
                            let icon_color = colors.icon_file;
                            SelectObject(hdc, state.font_main);
                            SetTextColor(hdc, COLORREF(icon_color & 0x00FFFFFF));
                            let mut icon_item_rect_mut = icon_item_rect;
                            DrawTextW(
                                hdc,
                                &mut to_wide_chars("📄"),
                                &mut icon_item_rect_mut,
                                DT_LEFT | DT_SINGLELINE | DT_VCENTER,
                            );
                        }
                        ResultType::Folder => {
                            let icon_color = colors.icon_folder;
                            SelectObject(hdc, state.font_main);
                            SetTextColor(hdc, COLORREF(icon_color & 0x00FFFFFF));
                            let mut icon_item_rect_mut = icon_item_rect;
                            DrawTextW(
                                hdc,
                                &mut to_wide_chars("📁"),
                                &mut icon_item_rect_mut,
                                DT_LEFT | DT_SINGLELINE | DT_VCENTER,
                            );
                        }
                    }

                    // Name (with reduced gap from icon)
                    let text_x = icon_x + ICON_SIZE + ICON_TEXT_GAP;
                    SelectObject(hdc, state.font_main);
                    SetTextColor(hdc, COLORREF(colors.text_primary & 0x00FFFFFF));

                    let name = truncate_with_ellipsis(&result.name, 35);
                    let name_rect = RECT {
                        left: text_x,
                        top: item_rect.top + 6,
                        right: item_rect.right - 8,
                        bottom: item_rect.top + 28,
                    };
                    let mut name_rect_mut = name_rect;
                    DrawTextW(
                        hdc,
                        &mut to_wide_chars(&name),
                        &mut name_rect_mut,
                        DT_LEFT | DT_SINGLELINE | DT_END_ELLIPSIS,
                    );

                    // Description
                    SelectObject(hdc, state.font_secondary);
                    SetTextColor(hdc, COLORREF(colors.text_secondary & 0x00FFFFFF));

                    let desc = truncate_with_ellipsis(&result.description, 40);
                    let desc_rect = RECT {
                        left: text_x,
                        top: item_rect.top + 28,
                        right: item_rect.right - 8,
                        bottom: item_rect.bottom - 4,
                    };
                    let mut desc_rect_mut = desc_rect;
                    DrawTextW(
                        hdc,
                        &mut to_wide_chars(&desc),
                        &mut desc_rect_mut,
                        DT_LEFT | DT_SINGLELINE | DT_END_ELLIPSIS,
                    );
                }

                y += ITEM_HEIGHT;
            }

            // Restore clipping (remove clip region)
            let _ = SelectClipRgn(hdc, None);
            let _ = DeleteObject(clip_region);
        }
    } else if !state.query.is_empty() {
        // No results message - Centered and clear
        SelectObject(hdc, state.font_secondary);
        SetTextColor(hdc, COLORREF(colors.text_muted & 0x00FFFFFF));

        let no_results_rect = RECT {
            left: PADDING,
            top: results_top + 40,
            right: rect.right - PADDING,
            bottom: results_top + 80,
        };
        let mut no_results_rect_mut = no_results_rect;

        // Draw formatted no results message centered
        use windows::Win32::Graphics::Gdi::DT_CENTER;
        DrawTextW(
            hdc,
            &mut to_wide_chars("No results found"),
            &mut no_results_rect_mut,
            DT_CENTER | DT_SINGLELINE | DT_VCENTER,
        );
    }

    // Cleanup
    let _ = DeleteObject(bg_brush);
    let _ = DeleteObject(input_bg_brush);
    let _ = DeleteObject(selection_brush);
    let _ = DeleteObject(hover_brush);
    let _ = DeleteObject(accent_brush);

    // Blit to screen
    let _ = BitBlt(hdc_screen, 0, 0, width, height, hdc_buffer, 0, 0, SRCCOPY);

    SelectObject(hdc_buffer, hbm_old);
    let _ = DeleteObject(hbm_buffer);
    let _ = DeleteDC(hdc_buffer);

    let _ = EndPaint(hwnd, &ps);
}

unsafe fn init_tray_icon(hwnd: HWND) {
    let mut nid = NOTIFYICONDATAW::default();
    nid.cbSize = mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = 1;
    nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    nid.uCallbackMessage = WM_TRAYICON;
    // Try to load embedded icon (ID 1), fallback to application icon
    nid.hIcon = LoadIconW(GetModuleHandleW(None).unwrap_or_default(), PCWSTR(1 as *const u16))
        .unwrap_or_else(|_| LoadIconW(None, IDI_APPLICATION).unwrap_or_default());
    
    let tip = "Rustle";
    for (i, c) in tip.encode_utf16().enumerate() {
        if i < nid.szTip.len() - 1 {
            nid.szTip[i] = c;
        }
    }
    let _ = Shell_NotifyIconW(NIM_ADD, &nid);
}

unsafe fn remove_tray_icon(hwnd: HWND) {
    let mut nid = NOTIFYICONDATAW::default();
    nid.cbSize = mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = 1;
    let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
}

unsafe fn calculate_cursor_from_x(hwnd: HWND, text: &str, target_x: i32, font: HFONT) -> usize {
    if target_x <= 0 { return 0; }
    
    let hdc = GetDC(hwnd);
    if hdc.is_invalid() { return 0; }
    
    let old_font = SelectObject(hdc, font);
    
    let wide: Vec<u16> = text.encode_utf16().collect();
    let mut best_idx = 0;
    let mut min_diff = i32::MAX;
    
    // Linear scan for closest character boundary
    for i in 0..=wide.len() {
        let mut size = SIZE::default();
        let _ = GetTextExtentPoint32W(hdc, &wide[0..i], &mut size);
        
        let diff = (size.cx - target_x).abs();
        if diff < min_diff {
            min_diff = diff;
            best_idx = i;
        } else if diff > min_diff {
            break; 
        }
    }
    
    SelectObject(hdc, old_font);
    ReleaseDC(hwnd, hdc);
    
    best_idx
}

fn to_wide_chars(s: &str) -> Vec<u16> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    OsStr::new(s).encode_wide().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_wide_chars() {
        let wide = to_wide_chars("Hello");
        assert_eq!(wide.len(), 5);
    }
}
