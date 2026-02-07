// Copyright 2026 hrzlgnm
// SPDX-License-Identifier: MIT-0
#![forbid(unsafe_code)]

mod tui_app;

use clap::Parser;

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
