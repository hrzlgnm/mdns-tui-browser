// Copyright 2026 hrzlgnm
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

mod daemon_status;
mod input;
mod logging;
mod models;
mod popup;
mod quick_filter;
mod scroll;
mod status_message;
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

    /// Override the default log file
    #[arg(
        long,
        value_name = "FILE",
        help = "Override the default log file",
        long_help = "Override the default log file (~/.local/state/mdns-tui-browser/mdns-tui-browser.log on Linux, ~/Library/Logs/mdns-tui-browser on macOS, %LOCALAPPDATA%\\mdns-tui-browser\\logs on Windows). Appended; rotated past 5 MiB with 3 backups kept. Without it, the app logs to the default file, or discards logs when that directory is unusable."
    )]
    log_file: Option<String>,

    /// Minimum log level recorded
    #[arg(
        long,
        value_name = "LEVEL",
        value_parser = parse_log_level,
        help = "Minimum log level recorded (off, error, warn, info, debug, trace)",
        long_help = "Minimum log level recorded (off, error, warn, info, debug, trace). When omitted, info and above is recorded."
    )]
    log_level: Option<log::LevelFilter>,
}

fn parse_log_level(value: &str) -> Result<log::LevelFilter, String> {
    use std::str::FromStr;
    log::LevelFilter::from_str(value).map_err(|_| format!("invalid log level: {value}"))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    if cli.load_state.is_none() && cli.no_ipv4 && cli.no_ipv6 {
        return Err("Cannot disable both IPv4 and IPv6. At least one must be enabled.".into());
    }

    // Install the file logger before the TUI takes over the terminal: a
    // bad explicit --log-file aborts with a clear error, an unusable
    // default log directory just disables logging.
    logging::init_logging(cli.log_file.as_deref(), cli.log_level)?;

    // Normalize service types (add .local. suffix if missing)
    let user_requested_service_types = cli
        .service_types
        .unwrap_or_default()
        .into_iter()
        .map(|service_type| tui_app::normalize_service_type(&service_type))
        .collect();

    // Validate interfaces before starting
    let interfaces = match cli.interfaces {
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
            Some(ifs)
        }
        None => None,
    };

    let disable_ipv4 = cli.no_ipv4;
    let disable_ipv6 = cli.no_ipv6;

    // Startup facts, so every log carries the exact run configuration.
    if let Some(path) = &cli.load_state {
        log::info!(
            "mdns-tui-browser: view-only mode, state file: {}",
            path.display()
        );
    } else {
        log::info!("mdns-tui-browser: live browsing");
    }
    log::info!(
        "mdns-tui-browser: service types: {:?}",
        user_requested_service_types
    );
    log::info!("mdns-tui-browser: interfaces: {:?}", interfaces);
    log::info!(
        "mdns-tui-browser: IPv4 {}, IPv6 {}",
        if disable_ipv4 { "disabled" } else { "enabled" },
        if disable_ipv6 { "disabled" } else { "enabled" }
    );

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
        interfaces,
        disable_ipv4,
        disable_ipv6,
        loaded_state,
        cli.log_file,
        cli.log_level.map(|level| level.to_string()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_file_flag_defaults_to_none_and_parses_path() {
        let cli = Cli::try_parse_from(["mdns-tui-browser"]).unwrap();
        assert!(cli.log_file.is_none());

        let cli = Cli::try_parse_from(["mdns-tui-browser", "--log-file", "/tmp/mdns.log"]).unwrap();
        assert_eq!(cli.log_file.as_deref(), Some("/tmp/mdns.log"));
    }

    #[test]
    fn test_log_level_flag_parses_case_insensitively() {
        let cli = Cli::try_parse_from(["mdns-tui-browser"]).unwrap();
        assert!(cli.log_level.is_none());

        let cli = Cli::try_parse_from(["mdns-tui-browser", "--log-level", "debug"]).unwrap();
        assert_eq!(cli.log_level, Some(log::LevelFilter::Debug));

        let cli = Cli::try_parse_from(["mdns-tui-browser", "--log-level", "TRACE"]).unwrap();
        assert_eq!(cli.log_level, Some(log::LevelFilter::Trace));
    }

    #[test]
    fn test_log_level_flag_rejects_invalid_value() {
        let result = Cli::try_parse_from(["mdns-tui-browser", "--log-level", "verbose"]);
        assert!(result.is_err(), "invalid level must not parse");
    }
}
