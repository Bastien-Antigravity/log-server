use crate::core::log_formatter::format_log_message;
use crate::utils::helpers::get_hostname;
use chrono::Local;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use tokio::sync::mpsc;

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
pub const COLOR_CYAN: &str = "\x1b[36m";

/// Returns colorized level string for console
pub fn colorize_level(level: &str) -> String {
    let color = match level {
        "DEBUG" => COLOR_CYAN,
        "INFO" | "LOGON" | "LOGOUT" => COLOR_GREEN,
        "WARNING" => COLOR_YELLOW,
        "ERROR" | "CRITICAL" => COLOR_RED,
        _ => return level.to_string(),
    };
    format!("{color}{level}{COLOR_RESET}")
}

/// Formats and prints an internal server log message (visual alignment only)
pub fn print_internal_log(
    level: &str,
    logger_name: &str,
    filename: &str,
    line: &str,
    message: &str,
) {
    let timestamp = Local::now().format("%Y-%m-%dT%H:%M:%S%.9fZ").to_string();

    let formatted = format_log_message(
        &timestamp,
        get_hostname(),
        logger_name,
        level,
        "log-server",
        filename,
        "internal",
        line,
        message,
        "",
        "",
        "",
        "",
        "",
        "",
        "",
    );

    // Apply console coloring consistent with LogWriter
    // Threshold updated to 73 (pre-level 63 + level 10)
    if formatted.len() > 73 {
        let level_part = formatted[63..73].trim();
        let colored = colorize_level(level_part);
        println!("{}{}{}", &formatted[..63], colored, &formatted[73..]);
    } else {
        println!("{formatted}");
    }

    // Write to file if enabled
    if let (Some(sender), Some(counter)) = (INTERNAL_SENDER.get(), INTERNAL_COUNTER.get()) {
        let seq = counter.fetch_add(1, Ordering::SeqCst);
        let file_msg = format!("{seq} {formatted}");
        let sender = sender.clone();
        tokio::spawn(async move {
            let _ = sender.send(file_msg).await;
        });
    }
}
