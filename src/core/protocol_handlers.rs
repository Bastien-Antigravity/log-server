//! Log message handling and processing
//!
//! Handles Cap'n Proto deserialization and message mapping to internal models.

use capnp::{message::ReaderOptions, serialize_packed};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::core::log_formatter::format_log_message;
use crate::models::log_entry::{LogEntry, LEVEL_STRINGS};

mod capnp_protocol {
    include!("../protocols/capnp/logger_msg.rs");
}

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
            .get_root::<capnp_protocol::logger_msg::Reader<'_>>()
            .map_err(|e| format!("invalid message format: {}", e))?;

        format_log_message_from_capnp(log_message)
            .map_err(|e| format!("message formatting failed: {}", e))?
    };

    // Send to writer with sequence number
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
    log_request: LogEntry,
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

/// Format log message from Cap'n Proto using centralized formatter
fn format_log_message_from_capnp(
    log_message: capnp_protocol::logger_msg::Reader<'_>,
) -> Result<String, Box<dyn std::error::Error>> {
    let timestamp = log_message.get_timestamp()?.to_str()?;
    let hostname = log_message.get_hostname()?.to_str()?;
    let logger_name = log_message.get_logger_name()?.to_str()?;
    let module = log_message.get_module()?.to_str()?;
    let level = LEVEL_STRINGS[log_message.get_level()? as usize];
    let filename = log_message.get_filename()?.to_str()?;
    let function_name = log_message.get_function_name()?.to_str()?;
    let line_number = log_message.get_line_number()?.to_str()?;
    let message = log_message.get_message()?.to_str()?;
    let path_name = log_message.get_path_name()?.to_str()?;
    let process_id = log_message.get_process_id()?.to_str()?;
    let process_name = log_message.get_process_name()?.to_str()?;
    let thread_id = log_message.get_thread_id()?.to_str()?;
    let thread_name = log_message.get_thread_name()?.to_str()?;
    let service_name = log_message.get_service_name()?.to_str()?;
    let stack_trace = log_message.get_stack_trace()?.to_str()?;

    Ok(format_log_message(
        timestamp,
        hostname,
        logger_name,
        level,
        module,
        filename,
        function_name,
        line_number,
        message,
        path_name,
        process_id,
        process_name,
        thread_id,
        thread_name,
        service_name,
        stack_trace,
    ))
}

//-----------------------------------------------------------------------------------------------

/// Format log message from gRPC using centralized formatter
fn format_log_message_from_grpc(
    log_message: LogEntry,
) -> Result<String, Box<dyn std::error::Error>> {
    Ok(format_log_message(
        &log_message.timestamp,
        &log_message.hostname,
        &log_message.logger_name,
        LEVEL_STRINGS[log_message.level as usize],
        &log_message.module,
        &log_message.filename,
        &log_message.function_name,
        &log_message.line_number,
        &log_message.message,
        &log_message.path_name,
        &log_message.process_id,
        &log_message.process_name,
        &log_message.thread_id,
        &log_message.thread_name,
        &log_message.service_name,
        &log_message.stack_trace,
    ))
}
