[![Scc Count Badge](https://sloc.xyz/github/hrzlgnm/mdns-tui-browser)](https://github.com/hrzlgnm/mdns-tui-browser)
# mDNS TUI Browser

A terminal-based mDNS service browser built with Rust, using `ratatui` for the TUI interface.

## Features

- 🖥️ **Interactive TUI**: Full terminal UI with keyboard navigation
- 📊 **Service Details**: Display IP address, port, and TXT records
- 🔄 **Real-time Updates**: Async architecture for live service discovery
- 🎛️ **Service Type Filtering**: Browse different service types separately

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
- <kbd>d</kbd> - Remove dead services
- <kbd>?</kbd> - Toggle help popup
- <kbd>q</kbd> or <kbd>Ctrl</kbd>+<kbd>c</kbd> - Quit the application

Press any key to close the help popup.



## Architecture

The application is built with:

- **Rust** - Systems programming language
- **ratatui** - Terminal UI framework
- **tokio** - Async runtime
- **crossterm** - Terminal handling
- **flume** - Async channel library for communication
- **mdns-sd** - mDNS service discovery library
- **clap** - Command line argument parsing library

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
