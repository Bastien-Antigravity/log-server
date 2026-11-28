//! ESSENTIAL PROCESS:
//! High-performance log message formatting with fixed-width column alignment,
//! optional ANSI coloring, and structured metadata tail.
//!
//! DATA FLOW:
//! 1. LogEntry fields -> format_log_message()
//! 2. Column formatting and optional ANSI color injection
//! 3. Formatted string output (console or disk persistence)
//!
//! KEY PARAMETERS:
//! - use_color: True for stdout ANSI coloration, false for disk persistence.
//! - Fixed-width column padding for timestamp, host, logger, level, file, func, line.

use crate::utils::helpers::truncate;
use std::fmt::Write;

/// Unified log message formatting - single-buffer allocation path used by both protocols
#[allow(clippy::too_many_arguments)]
pub fn format_log_message(
    timestamp: &str,
    hostname: &str,
    logger_name: &str,
    level: &str,
    module: &str,
    filename: &str,
    function_name: &str,
    line_number: &str,
    message: &str,
    path_name: &str,
    process_id: &str,
    process_name: &str,
    thread_id: &str,
    thread_name: &str,
    service_name: &str,
    stack_trace: &str,
    use_color: bool,
) -> String {
    let mut out = String::with_capacity(160 + message.len() + stack_trace.len());

    // 1. Basic 8 columns (Fixed width)
    let _ = write!(
        out,
        "{:<33} {:<12} {:<22} ",
        timestamp,
        truncate(hostname, 12),
        truncate(logger_name, 22),
    );
    if use_color {
        out.push_str(&crate::utils::terminal_ui::colorize_level(level));
    } else {
        let _ = write!(out, "{:<10}", truncate(level, 10));
    }
    let _ = write!(
        out,
        " {:<20} {:<25} {:<6} {}",
        truncate(filename, 20),
        truncate(function_name, 25),
        truncate(line_number, 6),
        message
    );

    // 2. Extra metadata (key=value) written directly into `out` without intermediate Vec<String>
    let mut has_meta = false;
    let mut append_meta = |key: &str, val: &str| {
        if !val.is_empty() {
            if !has_meta {
                out.push_str(" [metadata: ");
                has_meta = true;
            } else {
                out.push(' ');
            }
            out.push_str(key);
            out.push('=');
            out.push_str(val);
        }
    };

    append_meta("mod", module);
    append_meta("path", path_name);
    append_meta("pid", process_id);
    append_meta("pname", process_name);
    append_meta("tid", thread_id);
    append_meta("tname", thread_name);
    append_meta("svc", service_name);

    if !stack_trace.is_empty() {
        if !has_meta {
            out.push_str(" [metadata: stack=");
            has_meta = true;
        } else {
            out.push_str(" stack=");
        }
        for (i, line) in stack_trace.split('\n').enumerate() {
            if i > 0 {
                out.push_str(" | ");
            }
            out.push_str(line);
        }
    }

    if has_meta {
        out.push(']');
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_log_message_basic() {
        let timestamp = "2024-03-20T10:00:00.000Z";
        let formatted = format_log_message(
            timestamp,
            "host1",
            "logger1",
            "INFO",
            "mod1",
            "file1.rs",
            "func1",
            "10",
            "Hello World",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            false,
        );

        // Check fixed columns (33 timestamp + 12 host + 22 logger + 10 level + 20 file + 25 func + 6 line + message)
        assert!(formatted.starts_with(timestamp));
        assert!(formatted.contains("INFO"));
        assert!(formatted.contains("Hello World"));
        // Metadata mod=mod1 should be present
        assert!(formatted.contains("[metadata: mod=mod1]"));
    }

    #[test]
    fn test_format_log_message_with_metadata() {
        let formatted = format_log_message(
            "time", "host", "log", "DEBUG", "mod", "file", "func", "1", "msg", "path", "123",
            "pname", "456", "tname", "svc", "stack", false,
        );

        assert!(formatted.contains("path=path"));
        assert!(formatted.contains("pid=123"));
        assert!(formatted.contains("pname=pname"));
        assert!(formatted.contains("tid=456"));
        assert!(formatted.contains("tname=tname"));
        assert!(formatted.contains("svc=svc"));
        assert!(formatted.contains("stack=stack"));
    }

    #[test]
    fn test_truncation() {
        let long_host = "very-long-hostname-that-should-be-truncated";
        let formatted = format_log_message(
            "time", long_host, "log", "INFO", "", "", "", "", "msg", "", "", "", "", "", "", "",
            false,
        );

        // The helper truncate(hostname, 12) should be used
        assert!(formatted.contains("very-long-ho"));
        assert!(!formatted.contains(long_host));
    }

    // -----------------------------------------------------------------------------

    #[test]
    fn test_format_log_message_with_color() {
        let formatted = format_log_message(
            "time", "host", "log", "ERROR", "", "", "", "", "critical error occurred", "", "", "", "", "", "", "",
            true,
        );

        // Colorized ERROR should contain ANSI escape sequence (\x1b[)
        assert!(formatted.contains("\x1b["));
        assert!(formatted.contains("critical error occurred"));
    }
}
