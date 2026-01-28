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

## Controls (TUI Mode)

- `↑/↓` or `j/k` - Navigate services list
- `←/→` or `h/l` - Switch between service types  
- `PageUp/PageDown` or `Ctrl+u/Ctrl+d` - Scroll services list by page
- `b/f` or `Space` - Scroll services list by page
- `g/Home` - Jump to first service in list
- `G/End` - Jump to last service in list
- `q` or `Ctrl+C` - Quit the application



## Architecture

The application is built with:

- **Rust** - Systems programming language
- **ratatui** - Terminal UI framework
- **tokio** - Async runtime
- **crossterm** - Terminal handling

### Features

- **Extensible Design**: Built for real mDNS service discovery

## Project Structure

```
src/
├── main.rs       # Entry point with feature gating
├── tui_app.rs    # Full TUI implementation (optional)
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
