//! Search functionality for Rustle
//!
//! This module handles searching for applications and files across
//! the system. It indexes Start Menu shortcuts for applications and
//! traverses user directories for files.

#![allow(dead_code)]

use crate::config::SearchConfig;
use crate::error::Result;
use crate::utils::{display_name, is_shortcut, normalize_for_search};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use walkdir::WalkDir;

/// Represents a search result item
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Display name shown to the user
    pub name: String,

    /// Full path to the file or shortcut
    pub path: PathBuf,

    /// Type of result (Application, File, Folder)
    pub result_type: ResultType,

    /// Fuzzy match score (higher is better)
    pub score: i64,

    /// Optional description or path preview
    pub description: String,
}

/// Types of search results
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResultType {
    /// An application (Start Menu shortcut)
    Application,

    /// A regular file
    File,

    /// A directory/folder
    Folder,
}

impl ResultType {
    /// Returns a display string for the result type
    pub fn as_str(&self) -> &'static str {
        match self {
            ResultType::Application => "Application",
            ResultType::File => "File",
            ResultType::Folder => "Folder",
        }
    }

    /// Returns a priority for sorting (lower = higher priority)
    pub fn priority(&self) -> u8 {
        match self {
            ResultType::Application => 0,
            ResultType::Folder => 1,
            ResultType::File => 2,
        }
    }

    /// Returns a section header for UI display
    pub fn section_header(&self) -> &'static str {
        match self {
            ResultType::Application => "APPLICATIONS",
            ResultType::File => "FILES",
            ResultType::Folder => "FOLDERS",
        }
    }
}

/// Grouped search results for UI display
#[derive(Debug, Clone, Default)]
pub struct GroupedResults {
    pub applications: Vec<SearchResult>,
    pub folders: Vec<SearchResult>,
    pub files: Vec<SearchResult>,
}

impl GroupedResults {
    /// Returns total count of all results
    pub fn total_count(&self) -> usize {
        self.applications.len() + self.folders.len() + self.files.len()
    }

    /// Returns true if there are no results
    pub fn is_empty(&self) -> bool {
        self.applications.is_empty() && self.folders.is_empty() && self.files.is_empty()
    }

    /// Gets results by type
    pub fn get_by_type(&self, result_type: ResultType) -> &Vec<SearchResult> {
        match result_type {
            ResultType::Application => &self.applications,
            ResultType::Folder => &self.folders,
            ResultType::File => &self.files,
        }
    }

    /// Flattens results into a single vector with section markers
    pub fn flatten_with_sections(&self) -> Vec<FlatResult> {
        let mut results = Vec::new();

        if !self.applications.is_empty() {
            results.push(FlatResult::SectionHeader(ResultType::Application));
            for app in &self.applications {
                results.push(FlatResult::Item(app.clone()));
            }
        }

        if !self.folders.is_empty() {
            results.push(FlatResult::SectionHeader(ResultType::Folder));
            for folder in &self.folders {
                results.push(FlatResult::Item(folder.clone()));
            }
        }

        if !self.files.is_empty() {
            results.push(FlatResult::SectionHeader(ResultType::File));
            for file in &self.files {
                results.push(FlatResult::Item(file.clone()));
            }
        }

        results
    }
}

/// Flattened result for UI rendering (includes section headers)
#[derive(Debug, Clone)]
pub enum FlatResult {
    SectionHeader(ResultType),
    Item(SearchResult),
}

impl FlatResult {
    pub fn is_selectable(&self) -> bool {
        matches!(self, FlatResult::Item(_))
    }
}

/// The main search engine
///
/// Maintains an index of applications and provides search functionality
/// for both apps and files.
pub struct SearchEngine {
    /// Configuration for search behavior
    config: SearchConfig,

    /// Cached list of applications (Start Menu shortcuts)
    applications: Vec<SearchResult>,

    /// Fuzzy matcher instance
    matcher: SkimMatcherV2,

    /// Additional search paths (beyond config)
    extra_search_paths: Vec<PathBuf>,
}

impl SearchEngine {
    /// Creates a new search engine with the given configuration
    pub fn new(config: SearchConfig) -> Self {
        let mut engine = Self {
            config,
            applications: Vec::new(),
            matcher: SkimMatcherV2::default().smart_case(),
            extra_search_paths: Vec::new(),
        };

        // Add extra search paths for comprehensive search
        engine.init_extra_search_paths();

        // Index applications on creation
        if let Err(e) = engine.index_applications() {
            log::warn!("Failed to index some applications: {}", e);
        }

        engine
    }

    /// Initialize additional search paths including all available drives
    fn init_extra_search_paths(&mut self) {
        // User home directory
        if let Some(home) = dirs::home_dir() {
            self.extra_search_paths.push(home);
        }

        // Pictures
        if let Some(pics) = dirs::picture_dir() {
            self.extra_search_paths.push(pics);
        }

        // Videos
        if let Some(vids) = dirs::video_dir() {
            self.extra_search_paths.push(vids);
        }

        // Music
        if let Some(music) = dirs::audio_dir() {
            self.extra_search_paths.push(music);
        }

        // Common program locations on C drive
        let program_files = PathBuf::from(r"C:\Program Files");
        if program_files.exists() {
            self.extra_search_paths.push(program_files);
        }

        let program_files_x86 = PathBuf::from(r"C:\Program Files (x86)");
        if program_files_x86.exists() {
            self.extra_search_paths.push(program_files_x86);
        }

        // Use Windows API to get all logical drives (more reliable than checking exists())
        let available_drives = Self::get_logical_drives();

        log::info!(
            "Found {} logical drives: {:?}",
            available_drives.len(),
            available_drives
        );

        for drive_letter in available_drives {
            let drive = format!("{}:\\", drive_letter);
            let drive_path = PathBuf::from(&drive);

            // Skip A: and B: (usually floppy drives)
            if drive_letter == 'A' || drive_letter == 'B' {
                continue;
            }

            // Verify drive is accessible by checking if we can read its root
            if !Self::is_drive_accessible(&drive_path) {
                log::debug!("Drive {}: is not accessible, skipping", drive_letter);
                continue;
            }

            log::info!("Scanning drive {}: ({})", drive_letter, drive);

            // For non-C drives, scan ALL top-level directories (user's data drive)
            // For C drive, only scan specific system/user directories
            if drive_letter != 'C' {
                // Add the entire drive root - this will search all directories on the drive
                self.extra_search_paths.push(drive_path.clone());
                log::info!(
                    "  Added entire drive {}: for comprehensive search",
                    drive_letter
                );

                // Also try to enumerate top-level directories for better organization
                if let Ok(entries) = std::fs::read_dir(&drive_path) {
                    let mut top_level_dirs = 0;
                    for entry in entries.filter_map(|e| e.ok()) {
                        if let Ok(metadata) = entry.metadata() {
                            if metadata.is_dir() {
                                let dir_path = entry.path();
                                let dir_name =
                                    dir_path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                                // Skip system directories
                                let lower = dir_name.to_lowercase();
                                if !matches!(
                                    lower.as_str(),
                                    "$recycle.bin"
                                        | "system volume information"
                                        | "recovery"
                                        | "boot"
                                        | "perflogs"
                                        | "msocache"
                                        | "config.msi"
                                        | "intel"
                                        | "amd"
                                        | "nvidia"
                                        | "windows"
                                        | "program files"
                                        | "program files (x86)"
                                        | "programdata"
                                        | "users"
                                        | "windows.old"
                                ) {
                                    self.extra_search_paths.push(dir_path.clone());
                                    top_level_dirs += 1;
                                    log::debug!("  Added top-level dir: {}", dir_path.display());
                                }
                            }
                        }
                    }
                    if top_level_dirs > 0 {
                        log::info!(
                            "  Found {} top-level directories on drive {}:",
                            top_level_dirs,
                            drive_letter
                        );
                    }
                }
            } else {
                // For C: drive, only add common user directories (avoid system dirs)
                let common_dirs = [
                    "Users",
                    "Projects",
                    "Work",
                    "Development",
                    "Dev",
                    "Code",
                    "Documents",
                    "Downloads",
                    "Desktop",
                    "Games",
                    "Software",
                    "Programs",
                    "Apps",
                    "Data",
                ];

                let mut found_dirs_on_drive = 0;
                for dir_name in common_dirs {
                    let dir_path = drive_path.join(dir_name);
                    if dir_path.exists() && dir_path.is_dir() {
                        self.extra_search_paths.push(dir_path.clone());
                        found_dirs_on_drive += 1;
                        log::debug!("  Added: {}", dir_path.display());
                    }
                }

                if found_dirs_on_drive > 0 {
                    log::info!("  Found {} directories on drive C:", found_dirs_on_drive);
                }
            }
        }

        log::info!(
            "Initialized {} total search paths across all drives",
            self.extra_search_paths.len()
        );
    }

    /// Gets all logical drives using Windows API
    fn get_logical_drives() -> Vec<char> {
        use windows::Win32::Storage::FileSystem::GetLogicalDrives;

        unsafe {
            let drives = GetLogicalDrives();
            let mut result = Vec::new();

            // Each bit represents a drive (bit 0 = A:, bit 1 = B:, etc.)
            for i in 0..26 {
                if (drives & (1u32 << i)) != 0 {
                    let letter = (b'A' + i as u8) as char;
                    result.push(letter);
                }
            }

            result
        }
    }

    /// Checks if a drive is accessible (not just exists, but can be read)
    fn is_drive_accessible(drive_path: &Path) -> bool {
        // Try to read the drive root directory
        match std::fs::read_dir(drive_path) {
            Ok(_) => true,
            Err(e) => {
                log::debug!("Cannot access {}: {}", drive_path.display(), e);
                false
            }
        }
    }

    /// Indexes all Start Menu shortcuts
    fn index_applications(&mut self) -> Result<()> {
        self.applications.clear();

        // User Start Menu
        if let Some(start_menu) = dirs::data_dir() {
            let user_start = start_menu
                .parent()
                .map(|p| p.join("Roaming"))
                .map(|p| p.join("Microsoft"))
                .map(|p| p.join("Windows"))
                .map(|p| p.join("Start Menu"))
                .map(|p| p.join("Programs"));

            if let Some(path) = user_start {
                self.index_directory(&path, ResultType::Application)?;
            }
        }

        // System-wide Start Menu
        let system_start = PathBuf::from(r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs");
        if system_start.exists() {
            self.index_directory(&system_start, ResultType::Application)?;
        }

        log::info!("Indexed {} applications", self.applications.len());
        Ok(())
    }

    /// Indexes a directory for applications
    fn index_directory(&mut self, path: &Path, result_type: ResultType) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }

        for entry in WalkDir::new(path)
            .max_depth(5)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            if result_type == ResultType::Application && !is_shortcut(path) {
                continue;
            }

            let name = display_name(path);
            if should_skip_app(&name) {
                continue;
            }

            let description = path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();

            self.applications.push(SearchResult {
                name,
                path: path.to_path_buf(),
                result_type,
                score: 0,
                description,
            });
        }

        Ok(())
    }

    /// Performs an advanced search with the given query
    /// Returns grouped results for sectioned UI display
    pub fn search(&self, query: &str) -> GroupedResults {
        if query.is_empty() {
            return GroupedResults::default();
        }

        let normalized_query = normalize_for_search(query);
        let query_lower = query.to_lowercase();
        let mut grouped = GroupedResults::default();

        // Search applications (fast - in memory)
        for app in &self.applications {
            if let Some(score) = self.calculate_score(&app.name, &normalized_query, &query_lower) {
                let mut result = app.clone();
                result.score = score;
                grouped.applications.push(result);
            }
        }

        // Sort and limit applications
        grouped
            .applications
            .sort_unstable_by(|a, b| b.score.cmp(&a.score));
        grouped.applications.truncate(5);

        // Search files and folders if query is meaningful
        if query.len() >= 2 {
            self.search_files_and_folders(&normalized_query, &query_lower, &mut grouped);
        }

        // Remove duplicates by path (case-insensitive)
        let mut seen_paths: std::collections::HashSet<String> = std::collections::HashSet::new();

        grouped.folders.retain(|result| {
            let path_lower = result.path.to_string_lossy().to_lowercase();
            seen_paths.insert(path_lower)
        });

        grouped.files.retain(|result| {
            let path_lower = result.path.to_string_lossy().to_lowercase();
            seen_paths.insert(path_lower)
        });

        // Sort folders and files
        grouped
            .folders
            .sort_unstable_by(|a, b| b.score.cmp(&a.score));
        grouped.folders.truncate(4);

        grouped.files.sort_unstable_by(|a, b| b.score.cmp(&a.score));
        grouped.files.truncate(5);

        grouped
    }

    /// Advanced scoring algorithm
    fn calculate_score(
        &self,
        name: &str,
        normalized_query: &str,
        query_lower: &str,
    ) -> Option<i64> {
        let normalized_name = normalize_for_search(name);
        let name_lower = name.to_lowercase();

        // Get base fuzzy score
        let base_score = self
            .matcher
            .fuzzy_match(&normalized_name, normalized_query)?;

        let mut score = base_score;

        // Bonus for exact match
        if name_lower == *query_lower {
            score += 1000;
        }

        // Bonus for prefix match (name starts with query)
        if name_lower.starts_with(query_lower) {
            score += 500;
        }

        // Bonus for word-start match
        if name_lower
            .split_whitespace()
            .any(|word| word.starts_with(query_lower))
        {
            score += 200;
        }

        // Bonus for shorter names (more relevant)
        if name.len() < 20 {
            score += (20 - name.len() as i64) * 5;
        }

        // Penalty for very long names
        if name.len() > 50 {
            score -= 50;
        }

        Some(score)
    }

    /// Searches files and folders in all configured paths across all drives
    /// Uses parallel processing to search all drives simultaneously
    fn search_files_and_folders(
        &self,
        normalized_query: &str,
        query_lower: &str,
        grouped: &mut GroupedResults,
    ) {
        let max_per_path = 300; // Max files to check per search path

        // Combine config paths and extra paths
        let all_paths: Vec<PathBuf> = self
            .config
            .search_paths
            .iter()
            .chain(self.extra_search_paths.iter())
            .cloned()
            .collect();

        // Remove duplicates
        let mut seen_paths: std::collections::HashSet<String> = std::collections::HashSet::new();
        let unique_paths: Vec<PathBuf> = all_paths
            .into_iter()
            .filter(|path| {
                let path_str = path.to_string_lossy().to_lowercase();
                seen_paths.insert(path_str)
            })
            .collect();

        // Shared results collections for parallel access
        let folders = Mutex::new(Vec::<SearchResult>::new());
        let files = Mutex::new(Vec::<SearchResult>::new());

        // Clone query strings for parallel access
        let normalized_query = normalized_query.to_string();
        let query_lower = query_lower.to_string();

        // PARALLEL SEARCH: All drives searched simultaneously!
        unique_paths.par_iter().for_each(|search_path| {
            if !search_path.exists() {
                return;
            }

            // Determine search depth based on path type
            let path_str = search_path.to_string_lossy().to_lowercase();
            let is_drive_root =
                search_path.parent().is_none() || search_path.to_string_lossy().len() <= 3;
            let is_non_c_drive = !path_str.starts_with("c:");

            // For non-C drive roots, search 4 levels deep (comprehensive)
            // For C drive or user dirs, use 3 levels
            let max_depth = if is_drive_root && is_non_c_drive {
                4 // Deep search for data drives
            } else if is_drive_root {
                2 // Shallow for C: drive root
            } else {
                3 // Normal depth for user directories
            };

            let walker = WalkDir::new(search_path)
                .max_depth(max_depth)
                .follow_links(false)
                .into_iter()
                .filter_entry(|e| {
                    if let Some(name) = e.file_name().to_str() {
                        // Skip hidden files/directories
                        if name.starts_with('.') || name.starts_with('$') {
                            return false;
                        }
                        let lower = name.to_lowercase();
                        // Skip system and build directories
                        if matches!(
                            lower.as_str(),
                            "node_modules"
                                | ".git"
                                | "target"
                                | "__pycache__"
                                | ".cache"
                                | "appdata"
                                | "cache"
                                | "temp"
                                | "tmp"
                                | "$recycle.bin"
                                | "system volume information"
                                | "windows"
                                | "programdata"
                                | "recovery"
                                | "boot"
                                | "perflogs"
                                | "msocache"
                                | "config.msi"
                                | "intel"
                                | "amd"
                                | "nvidia"
                                | ".vs"
                                | ".idea"
                                | ".vscode"
                                | "bin"
                                | "obj"
                                | "debug"
                                | "release"
                                | "packages"
                                | ".nuget"
                                | "wpsystem"
                                | "windowsapps"
                                | "xboxgames"
                        ) {
                            return false;
                        }
                    }
                    true
                });

            let mut path_results_folders = Vec::new();
            let mut path_results_files = Vec::new();
            let mut path_checked = 0;

            // Check if the search path itself matches the query (for main folders)
            if search_path.is_dir() {
                let search_path_name = display_name(search_path);
                if let Some(score) =
                    self.calculate_score(&search_path_name, &normalized_query, &query_lower)
                {
                    let drive_boost = if is_non_c_drive { 100 } else { 0 };
                    let description = search_path
                        .parent()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default();

                    let result = SearchResult {
                        name: search_path_name,
                        path: search_path.clone(),
                        result_type: ResultType::Folder,
                        score: score + drive_boost,
                        description,
                    };
                    path_results_folders.push(result);
                }
            }

            for entry in walker.filter_map(|e| e.ok()) {
                path_checked += 1;

                // Limit per path
                if path_checked > max_per_path {
                    break;
                }

                let path = entry.path();
                // Don't skip the search path itself - we already checked it above
                if path == search_path {
                    continue;
                }

                let name = display_name(path);

                // Skip very short names for drive root searches
                if is_drive_root && name.len() < 2 {
                    continue;
                }

                if let Some(score) = self.calculate_score(&name, &normalized_query, &query_lower) {
                    // Boost score for files/folders on non-C drives
                    let drive_boost = if is_non_c_drive { 100 } else { 0 };

                    let description = path
                        .parent()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default();

                    let result = SearchResult {
                        name,
                        path: path.to_path_buf(),
                        result_type: if path.is_dir() {
                            ResultType::Folder
                        } else {
                            ResultType::File
                        },
                        score: score + drive_boost,
                        description,
                    };

                    if path.is_dir() {
                        path_results_folders.push(result);
                    } else {
                        path_results_files.push(result);
                    }
                }
            }

            // Merge results into shared collections
            if !path_results_folders.is_empty() {
                folders.lock().unwrap().extend(path_results_folders);
            }
            if !path_results_files.is_empty() {
                files.lock().unwrap().extend(path_results_files);
            }
        });

        // Collect results from parallel search
        let mut all_folders = folders.into_inner().unwrap();
        let mut all_files = files.into_inner().unwrap();

        // Remove duplicates by path (case-insensitive) - final deduplication
        let mut seen_paths: std::collections::HashSet<String> = std::collections::HashSet::new();

        all_folders.retain(|result| {
            let path_lower = result.path.to_string_lossy().to_lowercase();
            seen_paths.insert(path_lower)
        });

        all_files.retain(|result| {
            let path_lower = result.path.to_string_lossy().to_lowercase();
            seen_paths.insert(path_lower)
        });

        grouped.folders = all_folders;
        grouped.files = all_files;
    }

    /// Refreshes the application index
    pub fn refresh(&mut self) -> Result<()> {
        self.index_applications()
    }

    /// Returns the number of indexed applications
    pub fn application_count(&self) -> usize {
        self.applications.len()
    }
}

/// Checks if an application should be skipped during indexing
fn should_skip_app(name: &str) -> bool {
    let lower = name.to_lowercase();

    lower.contains("uninstall")
        || lower.contains("remove")
        || lower.contains("repair")
        || lower.contains("help")
        || lower.contains("readme")
        || lower.contains("manual")
        || lower.contains("documentation")
        || lower.contains("license")
        || lower.contains("website")
        || lower.contains("url")
        || lower == "about"
}

/// Creates a search engine with an Arc wrapper for thread-safe sharing
pub fn create_shared_engine(config: SearchConfig) -> Arc<SearchEngine> {
    Arc::new(SearchEngine::new(config))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_result_type_priority() {
        assert!(ResultType::Application.priority() < ResultType::File.priority());
    }

    #[test]
    fn test_should_skip_app() {
        assert!(should_skip_app("Uninstall Chrome"));
        assert!(should_skip_app("Remove App"));
        assert!(!should_skip_app("Google Chrome"));
        assert!(!should_skip_app("Visual Studio Code"));
    }

    #[test]
    fn test_grouped_results() {
        let grouped = GroupedResults::default();
        assert!(grouped.is_empty());
        assert_eq!(grouped.total_count(), 0);
    }
}
