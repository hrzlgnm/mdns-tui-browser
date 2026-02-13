// Copyright 2026 hrzlgnm
// SPDX-License-Identifier: MIT-0
#![forbid(unsafe_code)]

mod tui_app;

use std::collections::HashSet;

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
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

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

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(tui_app::run_tui(
        user_requested_service_types,
        cli.no_debounce,
        interfaces,
        available_interfaces,
    ))
}
