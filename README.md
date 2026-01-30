[![Scc Count Badge](https://sloc.xyz/github/hrzlgnm/mdns-tui-browser)](https://github.com/hrzlgnm/mdns-tui-browser)
![GitHub Downloads (all assets, all releases)](https://img.shields.io/github/downloads/hrzlgnm/mdns-tui-browser/total)
[![GitHub Downloads (all assets, latest release)](https://img.shields.io/github/downloads/hrzlgnm/mdns-tui-browser/latest/total)](https://github.com/hrzlgnm/mdns-tui-browser/releases/latest)
[![GitHub Release](https://img.shields.io/github/v/release/hrzlgnm/mdns-tui-browser)](https://github.com/hrzlgnm/mdns-tui-browser/releases/latest)
[![GitHub Release Date](https://img.shields.io/github/release-date/hrzlgnm/mdns-tui-browser)](https://github.com/hrzlgnm/mdns-tui-browser/releases/latest)
[![License: MIT](https://img.shields.io/github/license/hrzlgnm/mdns-tui-browser)](https://github.com/hrzlgnm/mdns-tui-browser/blob/main/LICENSE)
[![Build Status](https://img.shields.io/github/actions/workflow/status/hrzlgnm/mdns-tui-browser/ci.yml)](https://github.com/hrzlgnm/mdns-tui-browser/actions)
# mDNS TUI Browser

A terminal-based mDNS service browser built with Rust, using `ratatui` for the TUI interface.

## Features

- 🖥️ **Interactive TUI**: Full terminal UI with keyboard navigation
- 📊 **Service Details**: Display IP address, port, and TXT records
- 🔄 **Real-time Updates**: Async architecture for live service discovery
- 📈 **Live Metrics**: Comprehensive ServiceDaemon and application metrics
- 🎛️ **Service Type Filtering**: Browse different service types separately
- 📋 **Advanced Sorting**: Sort by Host, Type, Name, Port, Address, or Time in both directions

## Quick Start

```bash
cargo run
```

### Build Release Version
```bash
cargo build --release
```

### CLI Options

```bash
# Show version
mdns-tui-browser --version
mdns-tui-browser -V

# Show help
mdns-tui-browser --help
mdns-tui-browser -h
```

## Controls (TUI Mode)

- <kbd>↑</kbd>/<kbd>↓</kbd> or <kbd>j</kbd>/<kbd>k</kbd> - Navigate services list
- <kbd>←</kbd>/<kbd>→</kbd> or <kbd>h</kbd>/<kbd>l</kbd> - Switch between service types  
- <kbd>PageUp</kbd>/<kbd>PageDown</kbd> or <kbd>b</kbd>/<kbd>f</kbd> or <kbd>Space</kbd> - Scroll services list by page
- <kbd>Home</kbd>/<kbd>End</kbd> - Jump to first/last service in list
- <kbd>s</kbd> - Cycle sort field forward (Host → Type → Name → Port → Addr → Time)
- <kbd>S</kbd> - Cycle sort field backward (Time → Addr → Port → Name → Type → Host)
- <kbd>o</kbd> - Toggle sort direction (↑/↓)
- <kbd>d</kbd> - Remove offline services
- <kbd>m</kbd> - Show service metrics
- <kbd>?</kbd> - Toggle help popup
- <kbd>q</kbd> or <kbd>Ctrl</kbd>+<kbd>c</kbd> - Quit the application

## Architecture

The application is built with:

- **Rust** - Systems programming language
- **ratatui** - Terminal UI framework
- **tokio** - Async runtime
- **crossterm** - Terminal handling
- **flume** - Async channel library for communication
- **mdns-sd** - mDNS service discovery library
- **clap** - Command line argument parsing library
- **chrono** - Date and time handling for local timestamp display

### Features

- **Extensible Design**: Built for real mDNS service discovery

## Project Structure

```
src/
├── main.rs       # Entry point with cli argument handling
├── tui_app.rs    # Full TUI implementation
└── README.md     # This file
```

## Building

The project uses Cargo for building:

- `cargo run` - Run the TUI application
- `cargo build --release` - Build optimized release version

## Future Enhancements

- [ ] Service discovery configuration
- [ ] Export capabilities
- [ ] Service filtering and search
- [ ] Custom service type browsing
- [ ] Network interface selection

## License

MIT License
