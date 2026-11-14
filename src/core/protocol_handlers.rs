//! ESSENTIAL PROCESS:
//! Binary protocol decoding and message validation handlers. Unpacks Cap'n Proto
//! messages from SafeSocket frames and translates gRPC payloads into LogPacket instances.
//!
//! DATA FLOW:
//! 1. SafeSocket bytes -> Cap'n Proto Reader -> LogEntry
//! 2. gRPC ProtoLogRequest -> LogEntry
//! 3. LogEntry + Sequence Counter -> LogPacket -> writer_tx
//!
//! KEY PARAMETERS:
//! - identify_client_from_handshake: Extracts sender identity from initial connection.
//! - handle_tcp_message: Unpacks packed Cap'n Proto bytes into LogPacket.
//! - handle_grpc_message: Wraps gRPC LogEntry with sequence number into LogPacket.

use crate::models::log_entry::LogEntry;
use crate::models::log_packet::LogPacket;
use capnp::{message::ReaderOptions, serialize, serialize_packed};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::protocols::capnp::logger_msg as capnp_protocol;

/// Identify client from SafeSocket Hello handshake (Unpacked Cap'n Proto)
/// 
/// ⚠️ ARCHITECTURAL HACK: 
/// This implementation abuses the fact that 'HelloMsg' (from safe-socket) and 
/// 'LoggerMsg' (from log-server) both start with two Text fields at @0 and @1.
/// 
/// Layout Comparison:
/// - HelloMsg:  @0:fromName, @1:fromHost
/// - LoggerMsg: @0:timestamp, @1:hostname
/// 
/// This allows us to use the generated LoggerMsg reader to extract identity 
/// without needing to manually link or re-generate safe-socket schemas in Rust.
pub fn identify_client_from_handshake(data: &[u8]) -> Result<String, String> {
    // Note: Handshake is UNPACKED, so we use serialize::read_message
    let reader = serialize::read_message(&mut &data[..], ReaderOptions::new())
        .map_err(|e| format!("handshake read failed: {e}"))?;

    let hello_as_log = reader
        .get_root::<capnp_protocol::logger_msg::Reader<'_>>()
        .map_err(|e| format!("handshake root failed: {e}"))?;

    // Extraction (using aliases for clarity)
    let from_name = hello_as_log
        .get_timestamp() // @0 -> fromName
        .map_err(|e| format!("failed to extract 'fromName' from handshake: {e}"))?;

    let from_host = hello_as_log
        .get_hostname() // @1 -> fromHost
        .map_err(|e| format!("failed to extract 'fromHost' from handshake: {e}"))?;

    Ok(format!(
        "{}@{}",
        from_name.to_str().map_err(|e| format!("invalid UTF-8 in fromName: {e}"))?,
        from_host.to_str().map_err(|e| format!("invalid UTF-8 in fromHost: {e}"))?
    ))
}

// -----------------------------------------------------------------------------

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
        timestamp: log_msg
            .get_timestamp()
            .map_err(|e| e.to_string())?
            .to_string()
            .unwrap_or_default(),
        hostname: log_msg
            .get_hostname()
            .map_err(|e| e.to_string())?
            .to_string()
            .unwrap_or_default(),
        logger_name: log_msg
            .get_logger_name()
            .map_err(|e| e.to_string())?
            .to_string()
            .unwrap_or_default(),
        level: log_msg.get_level().map_err(|e| e.to_string())? as i32,
        module: log_msg
            .get_module()
            .map_err(|e| e.to_string())?
            .to_string()
            .unwrap_or_default(),
        filename: log_msg
            .get_filename()
            .map_err(|e| e.to_string())?
            .to_string()
            .unwrap_or_default(),
        function_name: log_msg
            .get_function_name()
            .map_err(|e| e.to_string())?
            .to_string()
            .unwrap_or_default(),
        line_number: log_msg
            .get_line_number()
            .map_err(|e| e.to_string())?
            .to_string()
            .unwrap_or_default(),
        message: log_msg
            .get_message()
            .map_err(|e| e.to_string())?
            .to_string()
            .unwrap_or_default(),
        path_name: log_msg
            .get_path_name()
            .map_err(|e| e.to_string())?
            .to_string()
            .unwrap_or_default(),
        process_id: log_msg
            .get_process_id()
            .map_err(|e| e.to_string())?
            .to_string()
            .unwrap_or_default(),
        process_name: log_msg
            .get_process_name()
            .map_err(|e| e.to_string())?
            .to_string()
            .unwrap_or_default(),
        thread_id: log_msg
            .get_thread_id()
            .map_err(|e| e.to_string())?
            .to_string()
            .unwrap_or_default(),
        thread_name: log_msg
            .get_thread_name()
            .map_err(|e| e.to_string())?
            .to_string()
            .unwrap_or_default(),
        service_name: log_msg
            .get_service_name()
            .map_err(|e| e.to_string())?
            .to_string()
            .unwrap_or_default(),
        stack_trace: log_msg
            .get_stack_trace()
            .map_err(|e| e.to_string())?
            .to_string()
            .unwrap_or_default(),
    };

    // 3. Send to writer with sequence number
    let sequence = sequence_counter.fetch_add(1, Ordering::SeqCst);
    writer_tx
        .send(LogPacket { sequence, entry })
        .await
        .map_err(|e| format!("failed to queue message: {e}"))?;

    Ok(())
}

// -----------------------------------------------------------------------------

/// Handle gRPC log message
pub async fn handle_grpc_message(
    log_request: LogEntry,
    writer_tx: mpsc::Sender<LogPacket>,
    sequence_counter: Arc<AtomicU64>,
) -> Result<(), String> {
    let sequence = sequence_counter.fetch_add(1, Ordering::SeqCst);
    writer_tx
        .send(LogPacket {
            sequence,
            entry: log_request,
        })
        .await
        .map_err(|e| format!("failed to queue gRPC message: {e}"))?;

    Ok(())
}

// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identify_client_from_valid_handshake() {
        let mut hello_msg = capnp::message::Builder::new_default();
        {
            let mut builder = hello_msg.init_root::<capnp_protocol::logger_msg::Builder>();
            builder.set_timestamp("gateway_service");
            builder.set_hostname("srv-node-1");
        }

        let mut buffer = Vec::new();
        capnp::serialize::write_message(&mut buffer, &hello_msg).unwrap();

        let identity = identify_client_from_handshake(&buffer).expect("valid handshake extraction");
        assert_eq!(identity, "gateway_service@srv-node-1");
    }

    // -----------------------------------------------------------------------------

    #[test]
    fn test_identify_client_from_corrupted_handshake() {
        let corrupted_data = [0u8, 1, 2, 3, 4, 5];
        let result = identify_client_from_handshake(&corrupted_data);
        assert!(result.is_err(), "Corrupted data should fail gracefully without panic");
    }
}
