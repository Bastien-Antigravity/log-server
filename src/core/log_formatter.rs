//! Log message formatting logic
//!
//! Implements the hybrid Logfmt style with fixed columns and extra metadata.

use crate::utils::helpers::truncate;

/// Unified log message formatting - used by both protocols
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
) -> String {
    // 1. Basic 8 columns (Fixed width)
    let base = format!(
        "{:<33} {:<12} {:<15} {:<8} {:<20} {:<25} {:<6} {}",
        timestamp,
        truncate(hostname, 12),
        truncate(logger_name, 15),
        truncate(level, 8),
        truncate(filename, 20),
        truncate(function_name, 25),
        truncate(line_number, 6),
        message
    );

    // 2. Extra metadata (key=value)
    let mut meta = Vec::new();
    if !module.is_empty() {
        meta.push(format!("mod={}", module));
    }
    if !path_name.is_empty() {
        meta.push(format!("path={}", path_name));
    }
    if !process_id.is_empty() {
        meta.push(format!("pid={}", process_id));
    }
    if !process_name.is_empty() {
        meta.push(format!("pname={}", process_name));
    }
    if !thread_id.is_empty() {
        meta.push(format!("tid={}", thread_id));
    }
    if !thread_name.is_empty() {
        meta.push(format!("tname={}", thread_name));
    }
    if !service_name.is_empty() {
        meta.push(format!("svc={}", service_name));
    }

    // Stack trace handling: append it last
    if !stack_trace.is_empty() {
        meta.push(format!("stack={}", stack_trace.replace('\n', " | ")));
    }

    if meta.is_empty() {
        base
    } else {
        format!("{} [metadata: {}]", base, meta.join(" "))
    }
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
        );

        // Check fixed columns (33 + 1 + 12 + 1 + 15 + 1 + 8 + 1 + 20 + 1 + 25 + 1 + 6 + 1 + length of message)
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
            "pname", "456", "tname", "svc", "stack",
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
        );

        // The helper truncate(hostname, 12) should be used
        assert!(formatted.contains("very-long-ho"));
        assert!(!formatted.contains(long_host));
    }
}
