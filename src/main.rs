// Copyright 2026 hrzlgnm
// SPDX-License-Identifier: MIT-0
#![forbid(unsafe_code)]

mod tui_app;

use clap::Parser;

#[derive(Parser)]
#[command(
    version = env!("CARGO_PKG_VERSION"),
    about = "A terminal-based mDNS service browser",
    after_help = "TUI Controls:\n  ?\t- Show/hide help popup with all key bindings\n  q\t- Quit the application\n\nFor complete key binding reference, press '?' in the application.",
)]
struct Cli {
    /// Service types to browse for (e.g., http, _http, _http._tcp, _http._tcp.local., printer.sub.http)
    /// Auto-completes missing protocol (defaults to _tcp) and .local. suffix
    /// Supports compact subtypes: printer.sub.http → _printer._sub._http._tcp.local.
    /// Supports full subtypes using the format _subtype._sub._service._protocol (e.g., _printer._sub._http._tcp, _airplay._sub._raop._tcp)
    #[arg(long, short, value_delimiter = ',')]
    service_types: Option<Vec<String>>,
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

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(tui_app::run_tui(user_requested_service_types))
}
