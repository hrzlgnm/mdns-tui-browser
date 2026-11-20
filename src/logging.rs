// Copyright 2026 hrzlgnm
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

//! File logging for diagnostics that must not print into the TUI.
//!
//! A minimal sink over the `log` facade with no new dependencies:
//! records go to a size-rotated file in the platform log directory,
//! and the terminal is never written to.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::Local;
use log::{LevelFilter, Log, Metadata, Record};

/// Live log size cap: past it the file rotates into numbered backups.
const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;

/// Backup generations kept when rotating.
const KEEP_LOG_GENERATIONS: u32 = 3;

/// Mutable logger state. Held in a global so `install` can reconfigure
/// the threshold even after the (process-wide, first-wins) logger has
/// been installed; this keeps level changes observable in tests.
struct LoggerState {
    file: Option<Mutex<RotatingLog>>,
    file_level: LevelFilter,
}

/// A plain lock rather than the tokio one used elsewhere: the `log`
/// facade calls synchronously from any thread, and a static needs a
/// const-constructible lock.
static LOGGER_STATE: Mutex<LoggerState> = Mutex::new(LoggerState {
    file: None,
    file_level: LevelFilter::Info,
});

/// Minimal log sink: records go to the size-rotated file at the given
/// level and above. The TUI must never be printed to (it would corrupt
/// the interface), so there is no stderr sink; when no file could be
/// opened, no records are kept.
struct Logger;

static LOGGER: Logger = Logger;

impl Logger {
    /// Installs the sink state, creating the process-wide logger on
    /// first use. A test binary shares one process-wide logger: first
    /// install wins, later ones are ignored, but the state is always
    /// updated.
    fn install(file: Option<RotatingLog>, log_level: Option<LevelFilter>) {
        let file_level = log_level.unwrap_or(LevelFilter::Info);
        if let Ok(mut state) = LOGGER_STATE.lock() {
            state.file = file.map(Mutex::new);
            state.file_level = file_level;
        }
        let _ = log::set_logger(&LOGGER);
        log::set_max_level(file_level);
    }
}

/// Last retained log write failure, if any. Global because the logger
/// is only reachable as a `dyn log::Log` once installed, so its state
/// cannot be accessed back through the trait object.
static LAST_LOG_ERROR: Mutex<Option<String>> = Mutex::new(None);

/// Takes the last retained log write failure, if any.
pub fn take_log_error() -> Option<String> {
    LAST_LOG_ERROR.lock().ok().and_then(|mut e| e.take())
}

/// Records a write failure as the logger would, for tests only.
#[cfg(test)]
pub fn set_log_error_for_test(message: &str) {
    if let Ok(mut last_error) = LAST_LOG_ERROR.lock() {
        *last_error = Some(message.to_string());
    }
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        LOGGER_STATE
            .lock()
            .map(|state| state.file.is_some() && metadata.level() <= state.file_level)
            .unwrap_or(false)
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format!(
            "{} [{}] {}\n",
            Local::now().format("%Y-%m-%dT%H:%M:%S%.3f"),
            record.level(),
            record.args()
        );
        if let Ok(state) = LOGGER_STATE.lock()
            && let Some(file) = &state.file
            && let Ok(mut file) = file.lock()
            && let Err(e) = file.write_line(&line)
            && let Ok(mut last_error) = LAST_LOG_ERROR.lock()
        {
            *last_error = Some(format!("log write failed: {e}"));
        }
    }

    fn flush(&self) {}
}

/// Default log directory: XDG state dir on Linux
/// (`$XDG_STATE_HOME/<app>`, else `~/.local/state/<app>`),
/// `~/Library/Logs/<app>` on macOS, `%LOCALAPPDATA%\<app>\logs` on
/// Windows, temp dir as a last resort.
fn default_log_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    if let Some(dir) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(dir).join(env!("CARGO_PKG_NAME")).join("logs");
    }
    #[cfg(target_os = "macos")]
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home)
            .join("Library")
            .join("Logs")
            .join(env!("CARGO_PKG_NAME"));
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    if let Some(state) = std::env::var_os("XDG_STATE_HOME")
        && !state.is_empty()
    {
        return PathBuf::from(state).join(env!("CARGO_PKG_NAME"));
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".local")
            .join("state")
            .join(env!("CARGO_PKG_NAME"));
    }
    std::env::temp_dir().join(env!("CARGO_PKG_NAME"))
}

/// Install logging before the app runs: an explicit `--log-file`
/// overrides the default file in the platform log directory. A bad
/// explicit path fails fast with a clear error; an unusable default
/// directory degrades to running without a logger, since the TUI must
/// not be printed to and browsing is worth more than diagnostics.
pub fn init_logging(
    log_file: Option<&str>,
    log_level: Option<LevelFilter>,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = match log_file {
        Some(p) => PathBuf::from(p),
        None => default_log_dir().join(format!("{}.log", env!("CARGO_PKG_NAME"))),
    };
    match RotatingLog::open(&path, MAX_LOG_BYTES, KEEP_LOG_GENERATIONS) {
        Ok(log) => {
            Logger::install(Some(log), log_level);
            Ok(())
        }
        Err(e) if log_file.is_some() => {
            Err(format!("cannot open log file {}: {e}", path.display()).into())
        }
        Err(_) => {
            Logger::install(None, log_level);
            Ok(())
        }
    }
}

/// Backup path `name.log.N` for generation `n`.
fn backup_path(path: &Path, n: u32) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{n}"));
    path.with_file_name(name)
}

/// Size-based log rotation: the live file plus `path.N` backups.
struct RotatingLog {
    /// `None` only while closed for renaming: Windows cannot rename an
    /// open file, so `rotate` drops the handle first.
    file: Option<File>,
    path: PathBuf,
    written: u64,
    max_bytes: u64,
    keep: u32,
}

impl RotatingLog {
    fn open(path: &Path, max_bytes: u64, keep: u32) -> std::io::Result<Self> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let mut log = Self {
            file: Some(Self::append(path)?),
            path: path.to_path_buf(),
            written: std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
            max_bytes,
            keep,
        };
        if log.written >= log.max_bytes {
            log.rotate()?;
        }
        Ok(log)
    }

    fn append(path: &Path) -> std::io::Result<File> {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
    }

    /// Append one line, rotating first when it would exceed the cap, so
    /// the live file stays bounded.
    fn write_line(&mut self, line: &str) -> std::io::Result<()> {
        if self.written + line.len() as u64 > self.max_bytes {
            self.rotate()?;
        }
        if let Some(f) = self.file.as_mut() {
            f.write_all(line.as_bytes())?;
            self.written += line.len() as u64;
        }
        Ok(())
    }

    /// Shift generations down (`log` -> `.1` -> `.2` ...) and reopen a
    /// fresh live file. Missing generations are skipped; rename targets
    /// are removed first for Windows, where rename will not replace.
    fn rotate(&mut self) -> std::io::Result<()> {
        // Close before renaming: Windows cannot rename an open file.
        self.file.take();
        for i in (1..=self.keep).rev() {
            let from = if i == 1 {
                self.path.clone()
            } else {
                backup_path(&self.path, i - 1)
            };
            if from.exists() {
                let to = backup_path(&self.path, i);
                let _ = std::fs::remove_file(&to);
                std::fs::rename(&from, &to)?;
            }
        }
        self.file = Some(Self::append(&self.path)?);
        self.written = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serializes tests that reconfigure the process-wide logger, avoiding
    /// cross-test races on the shared sink state.
    static TEST_LOG_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn temp_log_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "mdns-tui-browser-log-{name}-{}.log",
            std::process::id()
        ))
    }

    fn cleanup(path: &Path, keep: u32) {
        let _ = std::fs::remove_file(path);
        for n in 1..=keep {
            let _ = std::fs::remove_file(backup_path(path, n));
        }
    }

    #[test]
    fn test_logger_writes_records_to_file_with_timestamp_and_level() {
        let _guard = TEST_LOG_LOCK.lock().unwrap();
        let path = temp_log_path("facade");
        cleanup(&path, 3);

        let log = RotatingLog::open(&path, MAX_LOG_BYTES, KEEP_LOG_GENERATIONS).unwrap();
        Logger::install(Some(log), None);

        log::info!("hello {}", "log");

        let line = std::fs::read_to_string(&path).unwrap();
        let line = line.trim_end();
        // Line shape: `2026-11-20T16:24:35.745 [INFO] hello log`
        assert!(
            line.ends_with(" [INFO] hello log"),
            "unexpected line: {line}"
        );
        assert_eq!(
            line.len(),
            "YYYY-MM-DDTHH:MM:SS.mmm".len() + " [INFO] hello log".len()
        );
        cleanup(&path, 3);
    }

    #[test]
    fn test_logger_default_threshold_records_info_not_debug() {
        let _guard = TEST_LOG_LOCK.lock().unwrap();
        let path = temp_log_path("threshold");
        cleanup(&path, 3);

        let log = RotatingLog::open(&path, MAX_LOG_BYTES, KEEP_LOG_GENERATIONS).unwrap();
        Logger::install(Some(log), None);

        log::debug!("hidden");
        log::warn!("shown");

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(!content.contains("hidden"), "debug leaked: {content}");
        assert!(content.contains("[WARN] shown"), "warn missing: {content}");
        cleanup(&path, 3);
    }

    #[test]
    fn test_logger_without_file_discards_records() {
        let _guard = TEST_LOG_LOCK.lock().unwrap();

        Logger::install(None, None);

        // An unusable default log directory must degrade to silent
        // operation, never a panic.
        log::error!("discarded");
    }

    #[test]
    fn test_log_write_failure_is_retained_for_surfacing() {
        let _guard = TEST_LOG_LOCK.lock().unwrap();
        let path = temp_log_path("logfail");
        let backup = backup_path(&path, 1);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&backup);

        let log = RotatingLog::open(&path, 8, 1).unwrap();
        // Block the sole backup name with a directory: rotating the live
        // file onto it fails, whatever the user's privileges.
        std::fs::create_dir_all(&backup).unwrap();
        Logger::install(Some(log), None);

        // The oversized write has to rotate first, and the rotation
        // fails, so the sink records the write failure.
        log::error!("this write must fail");

        let err = take_log_error().expect("write failure retained");
        assert!(err.contains("log write failed"), "unexpected error: {err}");
        assert!(take_log_error().is_none(), "retention is take-once");

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&backup);
    }

    #[test]
    fn test_init_logging_explicit_path_creates_recording_sink() {
        let _guard = TEST_LOG_LOCK.lock().unwrap();
        let path = temp_log_path("init");
        cleanup(&path, 3);

        init_logging(path.to_str(), Some(log::LevelFilter::Info)).unwrap();

        log::warn!("init record");
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("[WARN] init record"), "missing: {content}");
        cleanup(&path, 3);
    }

    #[test]
    fn test_init_logging_bad_explicit_path_fails_clearly() {
        // The parent path exists as a regular file, so no log file can be
        // created beneath it.
        let blocker = temp_log_path("blocker");
        cleanup(&blocker, 3);
        std::fs::write(&blocker, "not a directory").unwrap();
        let bad = blocker.join("x.log");

        let err = init_logging(Some(bad.to_str().unwrap()), None).unwrap_err();
        assert!(
            err.to_string().contains("cannot open log file"),
            "unexpected error: {err}"
        );
        assert!(
            err.to_string().contains("x.log"),
            "error names the path: {err}"
        );

        cleanup(&blocker, 3);
    }

    #[test]
    fn test_rotating_log_rotates_on_open_when_file_exceeds_cap() {
        let path = temp_log_path("open-rotate");
        cleanup(&path, 3);
        std::fs::write(&path, "0123456789").unwrap();

        // Opening a file at or past the cap rotates it immediately; the
        // fresh live file only holds new records.
        let mut log = RotatingLog::open(&path, 8, 3).unwrap();
        assert_eq!(
            std::fs::read_to_string(backup_path(&path, 1)).unwrap(),
            "0123456789"
        );
        log.write_line("new\n").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new\n");
        cleanup(&path, 3);
    }

    #[test]
    fn test_rotating_log_appends_lines_to_file() {
        let path = temp_log_path("append");
        cleanup(&path, 3);

        let mut log = RotatingLog::open(&path, 1024, 3).unwrap();
        log.write_line("first\n").unwrap();
        log.write_line("second\n").unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "first\nsecond\n");
        cleanup(&path, 3);
    }

    #[test]
    fn test_rotating_log_rotates_when_line_exceeds_cap() {
        let path = temp_log_path("rotate");
        cleanup(&path, 3);

        let mut log = RotatingLog::open(&path, 16, 3).unwrap();
        log.write_line("first line\n").unwrap();

        // This line would exceed the cap: it lands in a fresh live file
        // while the old content moves to the .1 backup.
        log.write_line("second line too long\n").unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "second line too long\n"
        );
        assert_eq!(
            std::fs::read_to_string(backup_path(&path, 1)).unwrap(),
            "first line\n"
        );
        cleanup(&path, 3);
    }

    #[test]
    fn test_rotating_log_keeps_only_configured_generations() {
        let path = temp_log_path("generations");
        cleanup(&path, 3);

        let mut log = RotatingLog::open(&path, 12, 2).unwrap();
        log.write_line("line one\n").unwrap();
        log.write_line("line two longer\n").unwrap();
        log.write_line("line three longest\n").unwrap();
        log.write_line("line four longest yet\n").unwrap();

        assert!(backup_path(&path, 1).exists());
        assert!(backup_path(&path, 2).exists());
        assert!(!backup_path(&path, 3).exists());
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "line four longest yet\n"
        );
        cleanup(&path, 3);
    }
}
