[![Scc Count Badge](https://sloc.xyz/github/hrzlgnm/mdns-tui-browser)](https://github.com/hrzlgnm/mdns-tui-browser)
![GitHub Downloads (all assets, all releases)](https://img.shields.io/github/downloads/hrzlgnm/mdns-tui-browser/total)
[![GitHub Downloads (all assets, latest release)](https://img.shields.io/github/downloads/hrzlgnm/mdns-tui-browser/latest/total)](https://github.com/hrzlgnm/mdns-tui-browser/releases/latest)
[![GitHub Release](https://img.shields.io/github/v/release/hrzlgnm/mdns-tui-browser)](https://github.com/hrzlgnm/mdns-tui-browser/releases/latest)
[![GitHub Release Date](https://img.shields.io/github/release-date/hrzlgnm/mdns-tui-browser)](https://github.com/hrzlgnm/mdns-tui-browser/releases/latest)
[![AUR Version](https://img.shields.io/aur/version/mdns-tui-browser)](https://aur.archlinux.org/packages/mdns-tui-browser)
[![WinGet Package Version](https://img.shields.io/winget/v/hrzlgnm.mdns-tui-browser)](https://winstall.app/apps/hrzlgnm.mdns-tui-browser)
[![License: MIT](https://img.shields.io/github/license/hrzlgnm/mdns-tui-browser)](https://github.com/hrzlgnm/mdns-tui-browser/blob/main/LICENSE)
[![Build Status](https://img.shields.io/github/actions/workflow/status/hrzlgnm/mdns-tui-browser/ci.yml)](https://github.com/hrzlgnm/mdns-tui-browser/actions)
# mDNS TUI Browser

A terminal-based mDNS service browser built with Rust, using `ratatui` for the TUI interface. For an alternative running as a desktop app, check out [mDNS-Browser](https://github.com/hrzlgnm/mdns-browser).

![demo](./docs/assets/demo.gif)

## Features

- 🖥️ **Interactive TUI**: Full terminal UI with keyboard navigation
- 📊 **Service Details**: Display IP address, port, and TXT records
- 🔄 **Real-time Updates**: Async architecture for live service discovery
- 📈 **Live Metrics**: Comprehensive ServiceDaemon and application metrics
- 🎛️ **Service Type Filtering**: Browse different service types separately
- 🔍 **Quick Filter**: Text-based search across all service fields
- 📋 **Advanced Sorting**: Sort by Host, Type, Name, Port, Address, or Time in both directions
- ⚙️ **Custom Service Discovery**: Specify service types via CLI for hard-to-discover services
- 💾 **JSON State Dump**: Export complete application state to JSON with Ctrl+J

## Quick Start

```bash
cargo run
```

### Build Release Version
```bash
cargo build --release
```

### Install from AUR (Arch Linux)

You can install `mdns-tui-browser` from the Arch User Repository (AUR) using `yay`:

```bash
# Install using yay
yay -S mdns-tui-browser
```

This will install the latest version from the AUR and handle updates automatically with your regular system updates.

### Install using winget on Windows

You can install `mdns-tui-browser` from Winget Packages using `winget`:

```console
# Install using winget
winget install hrzlgnm.mdns-tui-browser
```

This will install the latest version from Winget Packages and handle updates automatically with your regular system updates.

### CLI Options

```bash
# Show version
mdns-tui-browser --version
mdns-tui-browser -V

# Show help
mdns-tui-browser --help
mdns-tui-browser -h

# Browse explicit service types, instead of auto-discovery
mdns-tui-browser --service-types "_http._tcp.local.,_ssh._tcp.local."
mdns-tui-browser -s "_printer._tcp.local."

# Browse with shortened service types (auto-completed)
mdns-tui-browser -s "_http,_ssh"
mdns-tui-browser -s "http,ssh"
mdns-tui-browser -s http -s ssh

# Use specific network interfaces
mdns-tui-browser -i en0
mdns-tui-browser --interfaces en0,eth0

# Disable IPv4 or IPv6 mDNS discovery
mdns-tui-browser --no-ipv4
mdns-tui-browser --no-ipv6

# Load state from a JSON file (view-only mode, no browsing)
mdns-tui-browser --load-state state-dump.json
mdns-tui-browser -l state-dump.json
```

#### Service Types Argument (`--service-types/-s`)

Service types to browse for (e.g., http, _http._tcp, printer). Auto-completes `(_)service`, `(_)sub`, `.(_)[tc|ud]p` and `.local.` suffix.

**Usage:**
- Can be specified multiple times
- Accepts comma-separated list of mDNS service types
- Auto-completes various service type formats automatically
- Examples: `http`, `_http._tcp`, `printer`

## JSON State Dump

The application can export the complete current state to a JSON file for debugging and analysis purposes, and can also load a previously exported state file for inspection.

### Usage

Press <kbd>Ctrl</kbd>+<kbd>J</kbd> in the TUI to trigger a state dump. The file will be saved with an ISO timestamp filename:

```text
20260207T120102.089898-state-dump.json
```

To load a previously exported state file for inspection (view-only mode, no browsing):

```bash
mdns-tui-browser --load-state state-dump.json
mdns-tui-browser -l state-dump.json
```

When loading a state file, no mDNS browsing is performed. The border color changes to a darker color to indicate view-only mode.

### JSON Structure

The exported JSON contains comprehensive information about the current application state:

```json
{
  "metadata": {
    "dumpTimestamp": "2026-02-07T00:40:10.950885Z",
    "applicationName": "mdns-tui-browser",
    "version": "1.11.1"
  },
  "services": [
    {
      "host": "my-device.local",
      "serviceType": "_http._tcp.local.",
      "addresses": ["192.168.1.100"],
      "port": 8080,
      "txtRecords": ["path=/api"],
      "isOnline": true,
      "createdAt": "2026-02-07T11:30:00.123456Z",
      "updatedAt": "2026-02-07T12:00:00.123456Z",
      "lastOnlineAt": "2026-02-07T12:00:00.123456Z",
      "sessionHistory": [
        {
          "startTime": "2026-02-07T00:39:53.820969Z",
          "endTime": "2026-02-07T00:39:55.869714Z"
        }
      ]
    }
  ],
  "serviceTypes": ["_http._tcp.local.", "_ssh._tcp.local."],
  "metrics": {
    "daemon_browse": 160,
    "daemon_cache_refresh_addr": 0,
    "daemon_cache_refresh_ptr": 0,
    "daemon_cache_refresh_srv_txt": 0,
    "daemon_cached_addr": 45,
    "daemon_cached_nsec": 14,
    "daemon_cached_ptr": 74,
    "daemon_cached_srv": 42,
    "daemon_cached_subtype": 1,
    "daemon_cached_txt": 42,
    "daemon_dns_registry_active": 0,
    "daemon_dns_registry_name_change": 0,
    "daemon_dns_registry_probe": 0,
    "daemon_dns_registry_timer": 0,
    "daemon_known_answer_suppression": 0,
    "daemon_timer": 5607,
    "service_types_discovered": 31,
    "services_discovered": 38,
    "services_marked_offline": 4,
    "services_updated": 14
  },
   "filters": {
    "query": "",
    "activeServiceTypes": ["_http._tcp.local."]
  },
  "sorting": {
    "field": "Host",
    "direction": "Ascending"
  }
}
```

## Controls

- <kbd>↑</kbd>/<kbd>↓</kbd> or <kbd>j</kbd>/<kbd>k</kbd> - Navigate services list
- <kbd>←</kbd>/<kbd>→</kbd> or <kbd>h</kbd>/<kbd>l</kbd> - Switch between service types  
- <kbd>H</kbd>/<kbd>L</kbd> - Page through service types
- <kbd>Ctrl</kbd>+<kbd>Home</kbd>/<kbd>End</kbd> - Jump to first/last service type
- <kbd>PageUp</kbd>/<kbd>PageDown</kbd> or <kbd>b</kbd>/<kbd>f</kbd> or <kbd>Space</kbd> - Scroll services list by page
- <kbd>Home</kbd>/<kbd>End</kbd> - Jump to first/last service in list
- <kbd>Enter</kbd> - Open service URL in browser
- <kbd>s</kbd> - Cycle sort field forward (Host → Type → Name → Port → Addr → Time)
- <kbd>S</kbd> - Cycle sort field backward (Time → Addr → Port → Name → Type → Host)
- <kbd>o</kbd> - Toggle sort direction (↑/↓)
- <kbd>/</kbd> - Enter quick filter mode (search across all service fields)
- <kbd>a</kbd> - Add new service type to browse
- <kbd>n</kbd> - Clear current filter
- <kbd>d</kbd> - Remove offline services
- <kbd>D</kbd> - Clear stale service types (service types with no services)
- <kbd>m</kbd> - Show service metrics
- <kbd>Ctrl</kbd>+<kbd>J</kbd> - Dump current state to JSON file
- <kbd>Ctrl</kbd>+<kbd>Z</kbd> - Suspend the application (Unix only)
- <kbd>?</kbd> - Toggle help popup
- <kbd>Shift</kbd>+<kbd>↑</kbd>/<kbd>↓</kbd> or <kbd>J</kbd>/<kbd>K</kbd> - Scroll service details
- <kbd>q</kbd> or <kbd>Ctrl</kbd>+<kbd>c</kbd> - Quit the application

### Service Details Controls
- <kbd>Shift</kbd>+<kbd>↑</kbd>/<kbd>↓</kbd> or <kbd>J</kbd>/<kbd>K</kbd> - Scroll service details

### Help Popup Controls
- <kbd>↑</kbd>/<kbd>↓</kbd> - Scroll help content
- <kbd>r</kbd> - Open release notes in browser
- Any other key - Close help popup

### Quick Filter Mode
When in filter mode (activated with <kbd>/</kbd>):
- <kbd>Enter</kbd> - Apply filter
- <kbd>Esc</kbd> - Cancel filter input
- <kbd>Backspace</kbd> - Delete last character
- Any other key - Type search text (case-insensitive)

#### Special Keywords
The quickfilter supports special keywords for filtering by service status:

- **`online`** - Shows online services AND services containing "online" in any field (fullname, host, service type, TXT records, etc.)
- **`offline`** - Shows offline services AND services containing "offline" in any field (fullname, host, service type, TXT records, etc.)
- **`flapping`** - Shows services that are flapping (detected based on session history - at least 3 sessions with at least half shorter than 5 minutes)
- **Case-insensitive**: `ONLINE`, `Online`, `offline`, `OFFLINE`, `FLAPPING`, `Flapping` all work
- **Hybrid mode**: Keywords match both by status AND text content
- **Combined queries**: `online http` shows online services with "http" in text fields

**Examples:**
```bash
# Press '/' then type:
online              # Show online services + services with "online" in text
offline             # Show offline services + services with "offline" in text
flapping            # Show flapping services
online http         # Show online HTTP services
offline printer     # Show offline printer services
online flapping     # Show services that are both online AND flapping
online offline      # Shows all services (both keywords present)
```

### Add Service Type Mode
When in add service type mode (activated with <kbd>a</kbd>):
- <kbd>Enter</kbd> - Add the service type and start browsing
- <kbd>Esc</kbd> - Cancel input
- <kbd>Backspace</kbd> - Delete last character
- Any other key - Type service type name

The service type is automatically normalized (e.g., `http` becomes `_http._tcp.local.`).

## Architecture

The application is built with:

- **Rust** - Systems programming language (Safe Rust only - no unsafe blocks allowed)
- **ratatui** - Terminal UI framework
- **tokio** - Async runtime
- **crossterm** - Terminal handling
- **flume** - Async channel library for communication
- **mdns-sd** - mDNS service discovery library
- **clap** - Command line argument parsing library
- **chrono** - Date and time handling for local timestamp display
- **serde** - Serialization framework for JSON export
- **serde_json** - JSON serialization support
- **if-addrs** - Network interface enumeration
- **nix** - Unix system calls (signal handling for Ctrl+Z suspend)
- **open** - Cross-platform library to open URLs in the browser

### Safety Policy

This project follows a **Safe Rust Only** policy:
- **No `unsafe` blocks** are permitted in the codebase
- All memory safety is guaranteed by Rust's compiler and runtime
- The codebase is configured to reject unsafe code
- This ensures maximum security and reliability for the mDNS service browser

### Features

- **Extensible Design**: Built for real mDNS service discovery

## Project Structure

```
src/
├── main.rs       # Entry point with CLI argument handling
├── tui_app.rs    # Main TUI application logic
├── input.rs      # User input handling (filter, service type)
├── popup.rs      # Popup UI components (help, metrics)
├── scroll.rs     # Scroll state management
├── models.rs     # Data models
└── terminal.rs  # Terminal management
```

## Building

The project uses Cargo for building:

- `cargo run` - Run the TUI application
- `cargo build --release` - Build optimized release version

## Build Provenance

Starting from **Release 1.6.0**, all release assets include cryptographic build provenance attestations generated by GitHub Actions. This provides verifiable evidence that the binaries were built from the official source code repository.

### What is Build Provenance?

Build provenance is a cryptographically signed attestation that answers:
- **Who** built the binary (GitHub Actions)
- **What** source code was used (specific commit hash)
- **How** the binary was built (build process and environment)
- **When** the binary was built (timestamp)

### Verifying Release Assets

You can verify the authenticity of release assets using the GitHub CLI:

```bash
# Using GitHub CLI
gh attestation verify mdns-tui-browser-<version>-Linux-x86_64.tar.gz --repo hrlzgnm/mdns-tui-browser
```

This ensures that the binaries you download are authentic and haven't been tampered with.

## Auditable Builds

Starting from **Release 1.6.0**, all release binaries are built with [cargo-auditable](https://github.com/rust-secure-code/cargo-auditable), which embeds detailed dependency information directly into the executable. This provides additional security benefits:

### What are Auditable Builds?

Auditable builds embed a complete dependency tree in JSON format within the binary, enabling:
- **Security Auditing**: Scan binaries for known vulnerabilities in their dependencies
- **Supply Chain Transparency**: Know exactly which crate versions were used
- **Zero Bookkeeping**: No separate dependency tracking required
- **Production Safety**: Verify that production binaries match expected dependencies

### How It Works

The `cargo auditable` command builds your application with additional metadata:
- Embeds JSON-formatted dependency information in a dedicated binary section
- Typically adds less than 4KB even to large dependency trees
- Works across all platforms (Linux, Windows, macOS, WebAssembly)

### Auditing Release Binaries

You can audit any release binary to verify its dependencies:

```bash
# Install required tools
cargo install cargo-audit

# Extract and audit dependencies from a release binary
cargo audit bin path/to/mdns-tui-browser
```

### Build Process

Release builds use auditable builds by default:
- **CI/CD builds**: Use `cargo auditable build --release`
- **Development builds**: Use standard `cargo build --release` for performance
- **Dependency tracking**: Automatic, no manual intervention required

This ensures that every release binary can be independently verified for security compliance and supply chain integrity.

## Future Enhancements

- [x] Export capabilities
- [x] Service filtering and search
- [x] Custom service type browsing
- [x] Network interface selection

## License

MIT License
