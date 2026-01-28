mod tui_app;

use clap::Parser;

#[derive(Parser)]
#[command(
    version = env!("CARGO_PKG_VERSION"),
)]
struct Cli {}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Cli::parse();

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(tui_app::run_tui())
}
