# Rustle

![Rust](https://img.shields.io/badge/Made_with-Rust-orange?logo=rust) ![Platform](https://img.shields.io/badge/Platform-Windows_11-blue?logo=windows) ![License](https://img.shields.io/badge/License-MIT-green)

Rustle is a high-performance search widget that lets you instantly find files and launch applications. Engineered entirely in Rust, it is designed to be a faster, lightweight, and modern alternative to the standard Windows search bar.

## ✨ Features

*   **Instant Search**: Zero-latency results powered by fuzzy matching logic.
*   **Keyboard Workflow**: 
    *   `Alt + Space` to summon
    *   `Up/Down` to navigate results
    *   `Enter` to open
    *   `Esc` to close
*   **Modern UI**: Glassmorphism aesthetic with native Windows 11 integration.
*   **System Tray**: Runs silently in the background with quick access controls.

## 🧠 Search Logic

Rustle prioritizes speed and relevance using a sophisticated multi-layered approach:

1.  **Fuzzy Matching**: Uses the **SkimMatcherV2** algorithm to handle typos and partial inputs intelligently (e.g., "vcode" matches "Visual Studio Code").
2.  **Parallel Indexing**: Leverages **Rayon** for multi-threaded traversal of the Start Menu and user directories (~150ms startup scan).
3.  **Smart Categorization**:
    *   **Applications** (Highest Priority)
    *   **Folders** (Medium Priority)
    *   **Files** (Standard Priority)

## 📦 Installation

### Pre-built Binaries
Download the latest `rustle.exe` from the [Releases](https://github.com/imsudip45/rustle/releases) page.

### From Source
```bash
git clone https://github.com/imsudip45/rustle.git
cd rustle
cargo build --release
```
The optimized binary will be in `target/release/`.

## 🚀 Usage

1.  Run the application.
2.  Press **Alt + Space** anywhere to open the search bar.
3.  Click and drag to select text, or use the clipboard shortcuts (`Ctrl+C`, `Ctrl+V`).

## License

This project is licensed under the [MIT License](LICENSE).
