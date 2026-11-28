//! ESSENTIAL PROCESS:
//! Binary protocol decoding and message validation handlers. Unpacks Cap'n Proto
//! messages from SafeSocket frames and translates gRPC payloads into LogPacket instances
//! for single-authority FIFO sequencing inside LogWriter.
//!
//! DATA FLOW:
//! 1. SafeSocket bytes -> Cap'n Proto Reader -> LogEntry
//! 2. gRPC ProtoLogRequest -> LogEntry
//! 3. LogEntry -> LogPacket -> writer_tx (sequenced authoritatively by LogWriter)
//!
//! KEY PARAMETERS:
//! - identify_client_from_handshake: Extracts sender identity from initial connection.
//! - handle_tcp_message: Unpacks packed Cap'n Proto bytes into LogPacket.
//! - handle_grpc_message: Enqueues gRPC LogEntry into writer_tx FIFO channel.

use crate::models::log_entry::LogEntry;
use crate::models::log_packet::LogPacket;
use capnp::{message::ReaderOptions, serialize, serialize_packed};
use tokio::sync::mpsc;

use crate::protocols::capnp::logger_msg as capnp_protocol;

//-----------------------------------------------------------------------------------------------

/// Dedicated Cap'n Proto reader for `safe-socket`'s unpacked `HelloMsg` handshake schema:
/// `struct HelloMsg { fromName @0 :Text; fromHost @1 :Text; fromAddress @2 :Text; toAddress @3 :Text; fromPublicIP @4 :Text; }`
pub mod hello_msg {
    #[derive(Clone, Copy)]
    pub struct Reader<'a> {
        reader: ::capnp::private::layout::StructReader<'a>,
    }

    impl<'a> From<::capnp::private::layout::StructReader<'a>> for Reader<'a> {
        fn from(reader: ::capnp::private::layout::StructReader<'a>) -> Self {
            Self { reader }
        }
    }

    impl<'a> ::capnp::traits::FromPointerReader<'a> for Reader<'a> {
        fn get_from_pointer(
            reader: &::capnp::private::layout::PointerReader<'a>,
            default: Option<&'a [::capnp::Word]>,
        ) -> ::capnp::Result<Self> {
            Ok(reader.get_struct(default)?.into())
        }
    }

    impl<'a> Reader<'a> {
        #[inline]
        pub fn get_from_name(self) -> ::capnp::Result<::capnp::text::Reader<'a>> {
            ::capnp::traits::FromPointerReader::get_from_pointer(
                &self.reader.get_pointer_field(0),
                None,
            )
        }

        #[inline]
        pub fn get_from_host(self) -> ::capnp::Result<::capnp::text::Reader<'a>> {
            ::capnp::traits::FromPointerReader::get_from_pointer(
                &self.reader.get_pointer_field(1),
                None,
            )
        }

        #[inline]
        pub fn get_from_address(self) -> ::capnp::Result<::capnp::text::Reader<'a>> {
            ::capnp::traits::FromPointerReader::get_from_pointer(
                &self.reader.get_pointer_field(2),
                None,
            )
        }
    }
}

//-----------------------------------------------------------------------------------------------

/// Identify client from SafeSocket Hello handshake (Unpacked Cap'n Proto `HelloMsg`)
pub fn identify_client_from_handshake(data: &[u8]) -> Result<String, String> {
    let reader = serialize::read_message(&mut &data[..], ReaderOptions::new())
        .map_err(|e| format!("handshake read failed: {e}"))?;

    let hello = reader
        .get_root::<hello_msg::Reader<'_>>()
        .map_err(|e| format!("handshake root failed: {e}"))?;

    let from_name = hello
        .get_from_name()
        .map_err(|e| format!("failed to extract 'fromName' from handshake: {e}"))?
        .to_str()
        .map_err(|e| format!("invalid UTF-8 in fromName: {e}"))?;

    let from_host = hello
        .get_from_host()
        .map_err(|e| format!("failed to extract 'fromHost' from handshake: {e}"))?
        .to_str()
        .map_err(|e| format!("invalid UTF-8 in fromHost: {e}"))?;

    Ok(format!("{from_name}@{from_host}"))
}

//-----------------------------------------------------------------------------------------------

#[inline]
fn read_capnp_text(res: ::capnp::Result<::capnp::text::Reader<'_>>) -> Result<String, String> {
    let reader = res.map_err(|e| e.to_string())?;
    if reader.is_empty() {
        return Ok(String::new());
    }
    Ok(reader.to_str().unwrap_or_default().to_owned())
}

/// Handle incoming TCP client message without intermediate Vec<u8> copy
pub async fn handle_tcp_message(
    data: &[u8],
    writer_tx: &mpsc::Sender<LogPacket>,
    _client_name: &str,
) -> Result<(), String> {
    // 1. Deserialization
    let mut slice = data;
    let reader = serialize_packed::read_message(&mut slice, ReaderOptions::new())
        .map_err(|e| format!("deserialization failed: {e}"))?;

    let log_msg = reader
        .get_root::<capnp_protocol::logger_msg::Reader<'_>>()
        .map_err(|e| format!("invalid message format: {e}"))?;

    // 2. Map to Internal Model (zero allocation for empty text fields)
    let entry = LogEntry {
        timestamp: read_capnp_text(log_msg.get_timestamp())?,
        hostname: read_capnp_text(log_msg.get_hostname())?,
        logger_name: read_capnp_text(log_msg.get_logger_name())?,
        level: log_msg.get_level().map_err(|e| e.to_string())? as i32,
        module: read_capnp_text(log_msg.get_module())?,
        filename: read_capnp_text(log_msg.get_filename())?,
        function_name: read_capnp_text(log_msg.get_function_name())?,
        line_number: read_capnp_text(log_msg.get_line_number())?,
        message: read_capnp_text(log_msg.get_message())?,
        path_name: read_capnp_text(log_msg.get_path_name())?,
        process_id: read_capnp_text(log_msg.get_process_id())?,
        process_name: read_capnp_text(log_msg.get_process_name())?,
        thread_id: read_capnp_text(log_msg.get_thread_id())?,
        thread_name: read_capnp_text(log_msg.get_thread_name())?,
        service_name: read_capnp_text(log_msg.get_service_name())?,
        stack_trace: read_capnp_text(log_msg.get_stack_trace())?,
    };

    // 3. Enqueue into FIFO writer channel (LogWriter is the single sequence authority)
    writer_tx
        .send(LogPacket {
            sequence: 0,
            entry,
        })
        .await
        .map_err(|e| format!("failed to queue message: {e}"))?;

    Ok(())
}

//-----------------------------------------------------------------------------------------------

/// Handle gRPC log message
pub async fn handle_grpc_message(
    log_request: LogEntry,
    writer_tx: &mpsc::Sender<LogPacket>,
) -> Result<(), String> {
    writer_tx
        .send(LogPacket {
            sequence: 0,
            entry: log_request,
        })
        .await
        .map_err(|e| format!("failed to queue gRPC message: {e}"))?;

    Ok(())
}

//-----------------------------------------------------------------------------------------------

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

    // -----------------------------------------------------------------------------

    #[tokio::test]
    async fn test_handle_tcp_message_packed_roundtrip() {
        let mut msg = capnp::message::Builder::new_default();
        {
            let mut b = msg.init_root::<capnp_protocol::logger_msg::Builder>();
            b.set_timestamp("2026-11-28T06:00:00Z");
            b.set_hostname("node-alpha");
            b.set_logger_name("orders");
            b.set_level(capnp_protocol::Level::Trade);
            b.set_message("filled 100 units");
            b.set_service_name("order_service");
        }
        let mut packed = Vec::new();
        serialize_packed::write_message(&mut packed, &msg).unwrap();

        let (tx, mut rx) = mpsc::channel(4);
        handle_tcp_message(&packed, &tx, "orders@node-alpha")
            .await
            .expect("packed Cap'n Proto decode should succeed");

        let pkt = rx.recv().await.expect("packet queued");
        assert_eq!(pkt.entry.hostname, "node-alpha");
        assert_eq!(pkt.entry.logger_name, "orders");
        assert_eq!(pkt.entry.level, 6); // TRADE
        assert_eq!(pkt.entry.message, "filled 100 units");
        assert_eq!(pkt.entry.service_name, "order_service");
        assert!(pkt.entry.stack_trace.is_empty());
    }
}
