//! Utility functions and helpers
//!
//! Common utilities used across the log server components.

pub mod helpers;

// Re-export commonly used utilities
pub use helpers::{
    create_log_folder,
    get_exec_parent_dir,
    get_utc_timestamp,
    validate_file_path,
    parse_sequence_number,
};