#![forbid(unsafe_code)]

mod tui_app;

use clap::Parser;

#[derive(Parser)]
#[command(
    version = env!("CARGO_PKG_VERSION"),
    about = "A terminal-based mDNS service browser",
    after_help = "TUI Controls:\n  ?\t- Show/hide help popup with all key bindings\n  q\t- Quit the application\n\nFor complete key binding reference, press '?' in the application.",
)]
struct Cli {}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Cli::parse();

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(tui_app::run_tui())
}
