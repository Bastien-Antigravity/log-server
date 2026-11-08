//! ESSENTIAL PROCESS:
//! Core data models representing unified log records across TCP and gRPC protocols.
//!
//! DATA FLOW:
//! 1. Ingested Protocol Structs -> LogEntry
//! 2. LogEntry -> Ring Buffer / Formatter / File Writer
//!
//! KEY PARAMETERS:
//! - LogEntry: Flattened struct with timestamp, host, logger, level, message, and metadata.
//! - LEVEL_STRINGS: Standard mapping array from numeric level to name string.

use serde::{Deserialize, Serialize};

/// Internal log request wrapper for internal use
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
