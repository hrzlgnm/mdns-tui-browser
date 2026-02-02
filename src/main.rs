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
    /// Additional service types to browse for (e.g., _http._tcp.local., _ssh._tcp.local., or _http._tcp, _ssh._tcp)
    #[arg(long, short, value_delimiter = ',')]
    service_types: Option<Vec<String>>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Normalize service types (add .local. suffix if missing)
    let additional_service_types = cli
        .service_types
        .unwrap_or_default()
        .into_iter()
        .map(|service_type| tui_app::normalize_service_type(&service_type))
        .collect();

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(tui_app::run_tui(additional_service_types))
}
