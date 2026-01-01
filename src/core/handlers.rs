//! Log message handling and processing
//!
//! Handles Cap'n Proto deserialization and message formatting.

use capnp::{message::ReaderOptions, serialize_packed};
use tokio::sync::mpsc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

mod logger_capnp {
    include!("../logger_capnp/logger_msg.rs");
}


// In handlers.rs
const LEVEL_STRINGS: [&str; 12] = [
    "NOTSET", "DEBUG", "STREAM", "INFO", "LOGON", "LOGOUT", "TRADE", "SCHEDULE", "REPORT",
    "WARNING", "ERROR", "CRITICAL",
];

//-----------------------------------------------------------------------------------------------

/// Handle incoming TCP client connection
pub async fn handle_tcp_message(
    data: Vec<u8>,
    writer_tx: mpsc::Sender<String>,
    sequence_counter: Arc<AtomicU64>,
    _client_name: &str,
) -> Result<(), String> {
    
    // Perform Cap'n Proto deserialization in the current thread
    let formatted_message = {
        // All Cap'n Proto work happens in this block
        let reader = serialize_packed::read_message(&mut &data[..], ReaderOptions::new())
            .map_err(|e| format!("deserialization failed: {}", e))?;
            
        let log_message = reader
            .get_root::<logger_capnp::logger_msg::Reader<'_>>()
            .map_err(|e| format!("invalid message format: {}", e))?;

        format_log_message_from_capnp(log_message)
            .map_err(|e| format!("message formatting failed: {}", e))?
    };

    // Send to writer with sequence number (this part is Send safe)
    let sequence = sequence_counter.fetch_add(1, Ordering::SeqCst);
    let final_message = format!("{} {}", sequence, formatted_message);
    
    writer_tx
        .send(final_message)
        .await
        .map_err(|e| format!("failed to queue message: {}", e))?;
        
    Ok(())
}

//-----------------------------------------------------------------------------------------------

/// Handle gRPC log message
pub async fn handle_grpc_message(
    log_request: crate::network::grpc_server::InternalLogRequest,
    writer_tx: mpsc::Sender<String>,
    sequence_counter: Arc<AtomicU64>,
) -> Result<(), String> {
    
    let formatted_message = format_log_message_from_grpc(log_request)
        .map_err(|e| format!("message formatting failed: {}", e))?;

    let sequence = sequence_counter.fetch_add(1, Ordering::SeqCst);
    let final_message = format!("{} {}", sequence, formatted_message);
    
    writer_tx
        .send(final_message)
        .await
        .map_err(|e| format!("failed to queue gRPC message: {}", e))?;
        
    Ok(())
}

//-----------------------------------------------------------------------------------------------
/// Unified log message formatting - used by both protocols
fn format_log_message(
    timestamp: &str,
    hostname: &str,
    logger_name: &str,
    level: &str,
    filename: &str,
    function_name: &str,
    line_number: &str,
    message: &str,
) -> String {

    format!(
        "{:<33} {:<12} {:<15} {:<8} {:<20} {:<25} {:<6} {}",
        timestamp, 
        truncate(hostname, 12), 
        truncate(logger_name, 15), 
        truncate(level, 8), 
        truncate(filename, 20), 
        truncate(function_name, 25), 
        truncate(line_number, 6),
        message
    )
}

// Helper to truncate long strings
fn truncate(s: &str, max_len: usize) -> &str {
    if s.len() > max_len {
        &s[..max_len]
    } else {
        s
    }
}

//-----------------------------------------------------------------------------------------------

/// Format log message from Cap'n Proto using unified formatter
fn format_log_message_from_capnp(
    log_message: logger_capnp::logger_msg::Reader<'_>,
) -> Result<String, Box<dyn std::error::Error>> {
    
    let timestamp = log_message.get_timestamp()?.to_str()?;
    let hostname = log_message.get_hostname()?.to_str()?;
    let logger_name = log_message.get_logger_name()?.to_str()?;
    let level = LEVEL_STRINGS[log_message.get_level()? as usize];
    let filename = log_message.get_filename()?.to_str()?;
    let function_name = log_message.get_function_name()?.to_str()?;
    let line_number = log_message.get_line_number()?.to_str()?;
    let message = log_message.get_message()?.to_str()?;

    Ok(format_log_message(
        timestamp, hostname, logger_name, level, filename, function_name, line_number, message
    ))
}

//-----------------------------------------------------------------------------------------------

/// Format log message from gRPC using unified formatter
fn format_log_message_from_grpc(
    log_message: crate::network::grpc_server::InternalLogRequest,
) -> Result<String, Box<dyn std::error::Error>> {
    
    Ok(format_log_message(
        &log_message.timestamp,
        &log_message.hostname,
        &log_message.logger_name,
        LEVEL_STRINGS[log_message.level as usize],
        &log_message.filename,
        &log_message.function_name,
        &log_message.line_number,
        &log_message.message,
    ))
}