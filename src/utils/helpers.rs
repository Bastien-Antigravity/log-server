//! Common utility functions

use std::path::PathBuf;




/// Create log folder if not exists
pub fn create_log_folder(path: &str) -> std::io::Result<()> {
    if std::fs::metadata(path).is_err() {
        std::fs::create_dir_all(path)?;
        println!("Created log directory: {}", path);
    }
    Ok(())
}

//-----------------------------------------------------------------------------------------------

/// Get executable parent directory
pub fn get_exec_parent_dir() -> PathBuf {
    match std::env::current_exe() {
        Ok(exe_path) => match exe_path.parent() {
            Some(parent) => parent.to_path_buf(),
            None => {
                eprintln!("Failed to get parent directory of the executable");
                std::process::exit(1);
            }
        },
        Err(e) => {
            eprintln!("Failed to get the executable path: {}", e);
            std::process::exit(1);
        }
    }
}


//-----------------------------------------------------------------------------------------------

/// Get current UTC timestamp as string
pub fn get_utc_timestamp() -> String {
    chrono::Utc::now().to_rfc3339()
}

//-----------------------------------------------------------------------------------------------

/// Validate file path is safe and within allowed directories
pub fn validate_file_path(path: &PathBuf, allowed_base: &PathBuf) -> Result<(), String> {
    if path.is_absolute() {
        return Err("Absolute paths are not allowed".to_string());
    }
    
    if path.components().count() > 10 {
        return Err("Path too deep".to_string());
    }
    
    // Check if path is within allowed base directory
    let full_path = allowed_base.join(path);
    if !full_path.starts_with(allowed_base) {
        return Err("Path traversal attempt detected".to_string());
    }
    
    Ok(())
}

//-----------------------------------------------------------------------------------------------

/// Parse sequence number from log message
pub fn parse_sequence_number(message: &str) -> Option<(u64, &str)> {
    if let Some((seq_str, rest)) = message.split_once(' ') {
        if let Ok(sequence) = seq_str.parse::<u64>() {
            return Some((sequence, rest));
        }
    }
    None
}