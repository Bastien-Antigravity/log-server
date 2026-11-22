//! ESSENTIAL PROCESS:
//! Terminal UI formatting, ANSI color palettes, and internal log-server self-logging.
//!
//! DATA FLOW:
//! 1. Internal logging calls -> print_internal_log()
//! 2. Colored formatting output to stdout
//! 3. Asynchronous FIFO queue dispatch to writer task for disk persistence
//!
//! KEY PARAMETERS:
//! - INTERNAL_SENDER: OnceLock channel to writer task for self-logging.
//! - INTERNAL_COUNTER: OnceLock atomic handle retained for signature compatibility.

use chrono::Utc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;

use crate::models::log_entry::LogEntry;
use crate::models::log_packet::LogPacket;
use crate::utils::helpers::{get_hostname, truncate};

static INTERNAL_SENDER: RwLock<Option<mpsc::Sender<LogPacket>>> = RwLock::new(None);
static INTERNAL_COUNTER: RwLock<Option<Arc<AtomicU64>>> = RwLock::new(None);
static DROPPED_INTERNAL_LOGS: AtomicU64 = AtomicU64::new(0);

//-----------------------------------------------------------------------------------------------

/// Initialize the internal logger with a sender and counter
pub fn set_internal_logger(sender: mpsc::Sender<LogPacket>, counter: Arc<AtomicU64>) {
    if let Ok(mut lock) = INTERNAL_SENDER.write() {
        *lock = Some(sender);
    }
    if let Ok(mut lock) = INTERNAL_COUNTER.write() {
        *lock = Some(counter);
    }
}

//-----------------------------------------------------------------------------------------------

/// Clear the internal logger during graceful shutdown to drop the writer channel sender
pub fn clear_internal_logger() {
    if let Ok(mut lock) = INTERNAL_SENDER.write() {
        *lock = None;
    }
    if let Ok(mut lock) = INTERNAL_COUNTER.write() {
        *lock = None;
    }
}

//-----------------------------------------------------------------------------------------------

/// Returns total count of internal log messages dropped due to channel saturation
pub fn get_dropped_internal_logs() -> u64 {
    DROPPED_INTERNAL_LOGS.load(Ordering::Relaxed)
}

//-----------------------------------------------------------------------------------------------

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

//-----------------------------------------------------------------------------------------------

/// Formats and prints an internal server log message (visual alignment only)
pub fn print_internal_log(
    level: &str,
    logger_name: &str,
    filename: &str,
    function_name: &str,
    line: &str,
    message: &str,
) {
    let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%S%.9fZ").to_string();

    let sender_opt = INTERNAL_SENDER.read().ok().and_then(|lock| lock.clone());

    // 1. Direct console output with functional coloring (only before writer is active)
    if sender_opt.is_none() {
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

    // 2. Map to internal LogEntry and queue for file write (sequenced authoritatively by LogWriter)
    if let Some(sender) = sender_opt {
        let entry = LogEntry {
            timestamp,
            hostname: get_hostname().to_string(),
            logger_name: logger_name.to_string(),
            level: match level {
                "NOTSET" => 0,
                "DEBUG" => 1,
                "STREAM" => 2,
                "INFO" => 3,
                "LOGON" => 4,
                "LOGOUT" => 5,
                "TRADE" => 6,
                "SCHEDULE" => 7,
                "REPORT" => 8,
                "WARNING" => 9,
                "ERROR" => 10,
                "CRITICAL" => 11,
                _ => 3, // Default to INFO (index 3)
            },
            module: "log-server".into(),
            filename: filename.to_string(),
            function_name: function_name.to_string(),
            line_number: line.to_string(),
            message: message.to_string(),
            ..Default::default()
        };

        let packet = LogPacket {
            sequence: 0,
            entry,
        };
        if let Err(tokio::sync::mpsc::error::TrySendError::Full(_)) = sender.try_send(packet) {
            let dropped = DROPPED_INTERNAL_LOGS.fetch_add(1, Ordering::Relaxed) + 1;
            if dropped == 1 || dropped.is_multiple_of(500) {
                eprintln!(
                    "[INTERNAL_LOGGER_PRESSURE] Internal logger channel saturated; dropped {} internal log records to prevent memory exhaustion",
                    dropped
                );
            }
        }
    }
}
