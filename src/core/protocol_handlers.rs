use crate::models::log_packet::LogPacket;
use crate::models::log_entry::LogEntry;
use capnp::{message::ReaderOptions, serialize, serialize_packed};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

#[allow(clippy::all)]
#[rustfmt::skip]
mod capnp_protocol {
    include!("../protocols/capnp/logger_msg.rs");
}

/// Identify client from SafeSocket Hello handshake (Unpacked Cap'n Proto)
/// This uses a "Layout Hack" by abusing the LoggerMsg reader, since both structs 
/// start with Text fields at indices 0 and 1.
pub fn identify_client_from_handshake(data: &[u8]) -> Result<String, String> {
    // Note: Handshake is UNPACKED, so we use serialize::read_message
    let reader = serialize::read_message(&mut &data[..], ReaderOptions::new())
        .map_err(|e| format!("handshake read failed: {e}"))?;
    
    let hello_as_log = reader.get_root::<capnp_protocol::logger_msg::Reader<'_>>()
        .map_err(|e| format!("handshake root failed: {e}"))?;
    
    // @0 timestamp (LoggerMsg) -> @0 fromName (HelloMsg)
    let from_name = hello_as_log.get_timestamp()
        .map_err(|e| format!("get fromName failed: {e}"))?;
        
    // @1 hostname (LoggerMsg) -> @1 fromHost (HelloMsg)
    let from_host = hello_as_log.get_hostname()
        .map_err(|e| format!("get fromHost failed: {e}"))?;
        
    Ok(format!("{}@{}", 
        from_name.to_str().map_err(|e| e.to_string())?, 
        from_host.to_str().map_err(|e| e.to_string())?))
}

//-----------------------------------------------------------------------------------------------

/// Handle incoming TCP client connection
pub async fn handle_tcp_message(
    data: Vec<u8>,
    writer_tx: mpsc::Sender<LogPacket>,
    sequence_counter: Arc<AtomicU64>,
    _client_name: &str,
) -> Result<(), String> {
    // 1. Deserialization
    let reader = serialize_packed::read_message(&mut &data[..], ReaderOptions::new())
        .map_err(|e| format!("deserialization failed: {e}"))?;

    let log_msg = reader
        .get_root::<capnp_protocol::logger_msg::Reader<'_>>()
        .map_err(|e| format!("invalid message format: {e}"))?;

    // 2. Map to Internal Model (Zero-String Policy: delayed formatting)
    let entry = LogEntry {
        timestamp: log_msg.get_timestamp().map_err(|e| e.to_string())?.to_string().unwrap_or_default(),
        hostname: log_msg.get_hostname().map_err(|e| e.to_string())?.to_string().unwrap_or_default(),
        logger_name: log_msg.get_logger_name().map_err(|e| e.to_string())?.to_string().unwrap_or_default(),
        level: log_msg.get_level().map_err(|e| e.to_string())? as i32,
        module: log_msg.get_module().map_err(|e| e.to_string())?.to_string().unwrap_or_default(),
        filename: log_msg.get_filename().map_err(|e| e.to_string())?.to_string().unwrap_or_default(),
        function_name: log_msg.get_function_name().map_err(|e| e.to_string())?.to_string().unwrap_or_default(),
        line_number: log_msg.get_line_number().map_err(|e| e.to_string())?.to_string().unwrap_or_default(),
        message: log_msg.get_message().map_err(|e| e.to_string())?.to_string().unwrap_or_default(),
        path_name: log_msg.get_path_name().map_err(|e| e.to_string())?.to_string().unwrap_or_default(),
        process_id: log_msg.get_process_id().map_err(|e| e.to_string())?.to_string().unwrap_or_default(),
        process_name: log_msg.get_process_name().map_err(|e| e.to_string())?.to_string().unwrap_or_default(),
        thread_id: log_msg.get_thread_id().map_err(|e| e.to_string())?.to_string().unwrap_or_default(),
        thread_name: log_msg.get_thread_name().map_err(|e| e.to_string())?.to_string().unwrap_or_default(),
        service_name: log_msg.get_service_name().map_err(|e| e.to_string())?.to_string().unwrap_or_default(),
        stack_trace: log_msg.get_stack_trace().map_err(|e| e.to_string())?.to_string().unwrap_or_default(),
    };

    // 3. Send to writer with sequence number
    let sequence = sequence_counter.fetch_add(1, Ordering::SeqCst);
    writer_tx
        .send(LogPacket { sequence, entry })
        .await
        .map_err(|e| format!("failed to queue message: {e}"))?;

    Ok(())
}

//-----------------------------------------------------------------------------------------------

/// Handle gRPC log message
pub async fn handle_grpc_message(
    log_request: LogEntry,
    writer_tx: mpsc::Sender<LogPacket>,
    sequence_counter: Arc<AtomicU64>,
) -> Result<(), String> {
    let sequence = sequence_counter.fetch_add(1, Ordering::SeqCst);
    writer_tx
        .send(LogPacket { sequence, entry: log_request })
        .await
        .map_err(|e| format!("failed to queue gRPC message: {e}"))?;

    Ok(())
}
