<p align="center">
  <img src="resources/app.ico" alt="Rustle Logo" width="128" height="128" />
</p>

# 🍃 Rustle

[![Rust](https://img.shields.io/badge/Built%20with-Rust-orange?style=for-the-badge&logo=rust)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/Platform-Windows%2010%2F11-blue?style=for-the-badge&logo=windows)](https://www.microsoft.com/windows)
[![License](https://img.shields.io/badge/License-MIT-yellow?style=for-the-badge)](https://opensource.org/licenses/MIT)

**A high-performance, native Windows search widget engineered in Rust.**

Rustle is a blazingly fast application launcher and file finder, written entirely in **Rust** to ensure memory safety and native performance. By leveraging the Win32 API directly through the distinct `windows-rs` crate, Rustle achieves an exceptionally low memory footprint and zero-latency response times, offering a modern "Spotlight-like" experience for Windows power users.

![Rustle Demo](resources/demo.gif)

## ✨ Why Rust?

- **Native Performance**: Compiled to machine code with zero runtime overhead.
- **Memory Safe**: Built on Rust's ownership model, guaranteeing memory safety without garbage collection.
- **Lightweight**: Uses ~15MB RAM, significantly less than Electron-based alternatives.
- **Direct Win32 API**: Bypasses heavy UI frameworks for raw speed and responsiveness.

## ✨ Features

- **Instant Search**: Results appear as you type with no perceptible lag.
- **Global Hotkey**: Summon instantly with `Alt + Space`.
- **Modern UI**: Custom-built rendering with glassmorphism, acrylic blur, and smooth animations.
- **Smart Indexing**: Recursive search for `Documents`, `Downloads`, and `Desktop`.
- **Clipboard Integration**: Seamless `Ctrl+C` (copy path) and `Ctrl+V` (paste query) support.
- **Keyboard Centric**: Full navigation optimized for keyboard-only usage.

## 📦 Installation

### Pre-built Binary
Download the latest `.exe` from the [**Releases Page**](https://github.com/imsudip45/rustle/releases).

### Build from Source
Ensure you have [Rust](https://rustup.rs/) installed.

```bash
git clone https://github.com/imsudip45/rustle.git
cd rustle
cargo build --release
```

The executable will be located at `target/release/rustle.exe`.

## 🚀 Usage

1. **Launch Rustle** (run `rustle.exe`).
2. Press **`Alt + Space`** to toggle the search bar.
3. **Type** to search for apps, files, or folders.
4. **Select** with `↑` / `↓` keys.
5. **Open** with `Enter`.

### Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Alt + Space` | Toggle Search |
| `Esc` | Close |
| `↑` / `↓` | Navigate Results |
| `Enter` | Open Selection |
| `Ctrl + C` | Copy path to clipboard |
| `Ctrl + V` | Paste into search bar |

## 🔧 Configuration

Rustle auto-generates a config file at `%APPDATA%\rustle\config.toml` on first run.

```toml
[hotkey]
modifier = "alt"  # alt, win, ctrl, shift
key = "space"

[appearance]
opacity = 0.98
width = 800
```

## 🛠️ Development

Rustle is a pure Rust project utilizing standard cargo workflows.

```bash
# Run in debug mode
cargo run

# Run tests
cargo test
```

## 🤝 Contributing

Contributions are welcome!
1. Fork it (`https://github.com/imsudip45/rustle/fork`)
2. Create your feature branch (`git checkout -b feature/cool-feature`)
3. Commit your changes (`git commit -m 'Add some cool feature'`)
4. Push to the branch (`git push origin feature/cool-feature`)
5. Create a new Pull Request

## 📝 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

**[imsudip45](https://github.com/imsudip45)**
