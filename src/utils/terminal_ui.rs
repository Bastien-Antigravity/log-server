use chrono::Local;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use tokio::sync::mpsc;

use crate::core::log_formatter::format_log_message;
use crate::utils::helpers::{get_hostname, truncate};

static INTERNAL_SENDER: OnceLock<mpsc::Sender<String>> = OnceLock::new();
static INTERNAL_COUNTER: OnceLock<Arc<AtomicU64>> = OnceLock::new();

/// Initialize the internal logger with a sender and counter
pub fn set_internal_logger(sender: mpsc::Sender<String>, counter: Arc<AtomicU64>) {
    let _ = INTERNAL_SENDER.set(sender);
    let _ = INTERNAL_COUNTER.set(counter);
}

// ANSI color codes
pub const COLOR_RESET: &str = "\x1b[0m";
pub const COLOR_RED: &str = "\x1b[31m";
pub const COLOR_GREEN: &str = "\x1b[32m";
pub const COLOR_YELLOW: &str = "\x1b[33m";
pub const COLOR_MAGENTA: &str = "\x1b[35m";
pub const COLOR_CYAN: &str = "\x1b[36m";

/// Returns colorized level string for console
pub fn colorize_level(level: &str) -> String {
    let color = match level {
        "DEBUG" | "STREAM" => COLOR_CYAN,
        "INFO" | "LOGON" | "LOGOUT" => COLOR_GREEN,
        "TRADE" | "SCHEDULE" | "REPORT" => COLOR_MAGENTA,
        "WARNING" => COLOR_YELLOW,
        "NOTSET" | "ERROR" | "CRITICAL" => COLOR_RED,
        _ => return format!("{:<10}", truncate(level, 10)),
    };
    format!("{color}{:<10}{COLOR_RESET}", truncate(level, 10))
}

/// Formats and prints an internal server log message (visual alignment only)
pub fn print_internal_log(
    level: &str,
    logger_name: &str,
    filename: &str,
    function_name: &str,
    line: &str,
    message: &str,
) {
    let timestamp = Local::now().format("%Y-%m-%dT%H:%M:%S%.9fZ").to_string();

    // 1. Generate formatted message for file (includes metadata)
    let file_formatted = format_log_message(
        &timestamp,
        get_hostname(),
        logger_name,
        level,
        "log-server",
        filename,
        function_name,
        line,
        message,
        "", "", "", "", "", "", "",
    );

    // 2. Direct console output with functional coloring (only if writer not yet active)
    if INTERNAL_SENDER.get().is_none() {
        let level_colored = colorize_level(level);
        println!(
            "{:<33} {:<12} {:<22} {} {:<20} {:<25} {:<6} {} [metadata: mod=log-server]",
            timestamp,
            truncate(get_hostname(), 12),
            truncate(logger_name, 22),
            level_colored,
            truncate(filename, 20),
            truncate(function_name, 25),
            truncate(line, 6),
            message
        );
    }

    // Write to file if enabled
    if let (Some(sender), Some(counter)) = (INTERNAL_SENDER.get(), INTERNAL_COUNTER.get()) {
        let seq = counter.fetch_add(1, Ordering::SeqCst);
        let final_file_msg = format!("{seq} {file_formatted}");
        let sender = sender.clone();
        tokio::spawn(async move {
            let _ = sender.send(final_file_msg).await;
        });
    }
}
