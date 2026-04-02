//! Core data models for the log server
//!
//! Centralized definitions to prevent circular dependencies between modules.

/// Internal log request wrapper for internal use
pub struct LogEntry {
    pub timestamp: String,
    pub hostname: String,
    pub logger_name: String,
    pub level: i32,
    pub module: String,
    pub filename: String,
    pub function_name: String,
    pub line_number: String,
    pub message: String,
    pub path_name: String,
    pub process_id: String,
    pub process_name: String,
    pub thread_id: String,
    pub thread_name: String,
    pub service_name: String,
    pub stack_trace: String,
}

/// Human-readable log level strings
pub const LEVEL_STRINGS: [&str; 12] = [
    "NOTSET", "DEBUG", "STREAM", "INFO", "LOGON", "LOGOUT", "TRADE", "SCHEDULE", "REPORT",
    "WARNING", "ERROR", "CRITICAL",
];
