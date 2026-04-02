//! Terminal UI and coloring utilities

use crate::core::log_formatter::format_log_message;
use chrono::Local;

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
    format!("{}{}{}", color, level, COLOR_RESET)
}

/// Formats and prints an internal server log message (visual alignment only)
pub fn print_internal_log(level: &str, logger_name: &str, filename: &str, line: &str, message: &str) {
    let timestamp = Local::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
    
    let formatted = format_log_message(
        &timestamp,
        "localhost",
        logger_name,
        level,
        "LogServer",
        filename,
        "internal",
        line,
        message,
        "", "", "", "", "", "", ""
    );

    // Apply console coloring consistent with LogWriter
    if formatted.len() > 71 {
        let level_part = formatted[63..71].trim();
        let colored = colorize_level(level_part);
        println!("{}{}{}", &formatted[..63], colored, &formatted[71..]);
    } else {
        println!("{}", formatted);
    }
}
