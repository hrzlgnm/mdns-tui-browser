// Copyright 2026 hrzlgnm
// SPDX-License-Identifier: MIT-0
#![forbid(unsafe_code)]

mod terminal;
mod tui_app;

use std::collections::HashSet;
use std::path::PathBuf;

use clap::Parser;
use if_addrs::get_if_addrs;

#[derive(Parser)]
#[command(
    version = env!("CARGO_PKG_VERSION"),
    about = "A terminal-based mDNS service browser",
    after_help = "TUI Controls:\n   ? - Show/hide help popup with all key bindings\n   q - Quit the application",
)]
struct Cli {
    /// Service types to browse for
    #[arg(
        long,
        short,
        value_delimiter = ',',
        long_help = "Service types to browse for (e.g., http, _http._tcp, printer)\nAuto-completes (_)service, (_)sub, .(_)[tc|ud]p and .local. suffix"
    )]
    service_types: Option<Vec<String>>,

    /// Disable debouncing of flapping services
    #[arg(
        long,
        help = "Disable automatic debouncing of flapping services for debugging"
    )]
    no_debounce: bool,

    /// Network interfaces to use for mDNS discovery
    #[arg(
        long,
        short,
        value_delimiter = ',',
        help = "Network interfaces to use for mDNS discovery (e.g., en0, eth0)"
    )]
    interfaces: Option<Vec<String>>,

    /// Disable IPv4 mDNS discovery
    #[arg(long, help = "Disable IPv4 mDNS discovery")]
    no_ipv4: bool,

    /// Disable IPv6 mDNS discovery
    #[arg(long, help = "Disable IPv6 mDNS discovery")]
    no_ipv6: bool,

    /// Load state from a JSON file (view-only mode, no browsing)
    #[arg(
        long,
        short,
        help = "Load state from a JSON file for inspection (view-only mode, no browsing)"
    )]
    load_state: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    if cli.load_state.is_none() && cli.no_ipv4 && cli.no_ipv6 {
        return Err("Cannot disable both IPv4 and IPv6. At least one must be enabled.".into());
    }

    // Normalize service types (add .local. suffix if missing)
    let user_requested_service_types = cli
        .service_types
        .unwrap_or_default()
        .into_iter()
        .map(|service_type| tui_app::normalize_service_type(&service_type))
        .collect();

    // Validate interfaces before starting
    let (interfaces, available_interfaces) = match cli.interfaces {
        Some(ifs) => {
            let available: HashSet<String> = get_if_addrs()
                .map_err(|e| format!("Failed to get network interfaces: {}", e))?
                .iter()
                .map(|i| i.name.clone())
                .collect();

            let mut sorted: Vec<_> = available.iter().collect();
            sorted.sort();

            for interface in &ifs {
                if !available.contains(interface) {
                    return Err(format!(
                        "Interface '{}' not found. Available interfaces: {}",
                        interface,
                        sorted
                            .iter()
                            .map(|s| s.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                    .into());
                }
            }
            (Some(ifs), Some(available.into_iter().collect::<Vec<_>>()))
        }
        None => (None, None),
    };

    let disable_ipv4 = cli.no_ipv4;
    let disable_ipv6 = cli.no_ipv6;

    let loaded_state: Option<String> = cli
        .load_state
        .map(|path| {
            std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read state file '{}': {}", path.display(), e))
        })
        .transpose()?;

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(tui_app::run_tui(
        user_requested_service_types,
        cli.no_debounce,
        interfaces,
        available_interfaces,
        disable_ipv4,
        disable_ipv6,
        loaded_state,
    ))
}
