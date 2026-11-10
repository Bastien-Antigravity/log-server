//! ESSENTIAL PROCESS:
//! Constructs exhaustive, compile-time embedded Exchange Contracts directly
//! from log-server schemas (log_service.proto, logger.capnp, and SafeSocket framing)
//! and publishes ServiceSpec metadata to web-interface.

use serde_json::{json, Value};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::config::config::Config;

pub const LOG_SERVICE_PROTO: &str = include_str!("../schema/proto/log_service.proto");
pub const LOGGER_CAPNP_SCHEMA: &str = include_str!("../schema/capnp/logger.capnp");

//-----------------------------------------------------------------------------------------------

fn display_host(host: &str) -> &str {
    if host.is_empty() || host == "0.0.0.0" || host == "::" {
        "127.0.0.1"
    } else {
        host
    }
}

/// Build exhaustive list of log-server exchange contracts anchored to runtime config & real schema files.
pub fn build_exchange_contracts_value(config: &Config, ring_capacity: usize) -> Value {
    let host = display_host(&config.host);
    let tcp_addr = format!("{}:{}", host, config.port);
    let grpc_addr = format!("{}:{}", host, config.grpc_port);

    let safesocket_spec = [
        "1. Connection Establishment (SafeSocket TCP Handshake / ADR-001):",
        "   - Frame Prefix: 4-byte unsigned big-endian integer length (max 10 MB).",
        "   - Zero-length Frame (0x00000000): Keepalive heartbeat; resets idle timeout without deserialization.",
        "   - Frame #1 (Unpacked Cap'n Proto HelloMsg):",
        "     struct HelloMsg { fromName @0 :Text; fromHost @1 :Text; fromAddress @2 :Text; toAddress @3 :Text; fromPublicIP @4 :Text; }",
        "   - Subsequent Frames (Packed Cap'n Proto LoggerMsg):",
        "     Deserialized via capnp::serialize_packed -> sequenced by AtomicU64 -> LogWriter BTreeMap reorder buffer -> disk & LogRingBuffer.",
    ]
    .join("\n");

    json!([
        {
            "id": "safesocket",
            "type": "SafeSocket TCP Protocol & ADR-001",
            "tabLabel": "Protocol & ADR-001",
            "icon": "fa-sitemap",
            "direction": "inbound",
            "transport": "TCP + SafeSocket 4-Byte BE Framing",
            "address": tcp_addr,
            "port": config.port,
            "framing": "4-byte BE uint32 length header + unpacked HelloMsg handshake + packed Cap'n Proto body",
            "auth": "SafeSocket HelloMsg identity extraction (fromName@fromHost)",
            "summary": format!("High-throughput asynchronous TCP log ingestor with 10MB frame guard, 0-len heartbeats, and sequence-ordered ring buffer (capacity {}).", ring_capacity),
            "source": "log-server/src/transport/safe_socket.rs + servers/tcp_server.rs",
            "peers": [
                { "service": "universal-logger (Go/Python/Rust)", "role": "producer", "description": "Streams packed Cap'n Proto LoggerMsg batches from every microservice" },
                { "service": "config_server", "role": "producer", "description": "Ships structured logs over SafeSocket TCP" },
                { "service": "watchdog_agent", "role": "producer", "description": "Ships supervisor & process lifecycle logs over SafeSocket TCP" },
                { "service": "notif_server", "role": "producer", "description": "Ships dispatcher logs (loopback-isolated [NS-1])" },
                { "service": "tele_remote", "role": "producer", "description": "Ships Telegram bot gateway logs over SafeSocket TCP" },
                { "service": "web_interface", "role": "producer", "description": "Ships dashboard HTTP & router logs over SafeSocket TCP" },
                { "service": "rag_engine", "role": "producer", "description": "Ships indexing & semantic search logs over SafeSocket TCP" },
                { "service": "agent_squad", "role": "producer", "description": "Ships autonomous orchestrator logs over SafeSocket TCP" }
            ],
            "operations": [
                { "name": "HelloHandshake", "kind": "FRAME", "target": "Unpacked HelloMsg (frame #1)", "input": "fromName @0, fromHost @1, fromAddress @2, toAddress @3, fromPublicIP @4", "output": "Client identity bound as fromName@fromHost", "description": "Initial SafeSocket client handshake frame" },
                { "name": "HeartbeatFrame", "kind": "FRAME", "target": "uint32(0x00000000)", "input": "4 zero bytes (slen == 0)", "output": "Timeout reset", "description": "Zero-allocation keepalive packet" },
                { "name": "LogPacketFrame", "kind": "STREAM", "target": "Packed LoggerMsg", "input": "len_be + serialize_packed(LoggerMsg)", "output": "LogPacket -> RingBuffer & rotated log files", "description": "Ordered log ingestion packet (max 10 MB)" }
            ],
            "schemaFormat": "wire",
            "schema": safesocket_spec
        },
        {
            "id": "proto",
            "type": "gRPC / Protobuf API",
            "tabLabel": "Protobuf API Spec",
            "icon": "fa-code",
            "direction": "inbound",
            "transport": "HTTP/2 + Tonic gRPC (Protobuf v3)",
            "address": grpc_addr,
            "port": config.grpc_port,
            "framing": "HTTP/2 DATA length-prefixed Protobuf messages (logservice.LogBridge)",
            "auth": "Internal insecure HTTP/2 gRPC listener",
            "summary": "Real-time bidirectional/streaming gRPC gateway enabling log ingestion, ring-buffer tail queries, live log streaming, and self-describing contract reflection.",
            "source": "log-server/src/schema/proto/log_service.proto (include_str!)",
            "peers": [
                { "service": "web_interface", "role": "consumer", "description": "Proxies GetTailLogs -> REST JSON, StreamLogs -> SSE, GetExchangeContracts -> /api/v1/system/logs/contracts" },
                { "service": "gRPC Log Producers", "role": "producer", "description": "Optional unary LogMessage ingestion over HTTP/2" }
            ],
            "operations": [
                { "name": "LogMessage", "kind": "RPC", "target": "logservice.LogBridge/LogMessage", "input": "LogRequest", "output": "LogResponse { success }", "description": "Ingest single structured log message via gRPC" },
                { "name": "GetTailLogs", "kind": "RPC", "target": "logservice.LogBridge/GetTailLogs", "input": "TailLogsRequest { limit, service_name, min_level }", "output": "TailLogsResponse { logs[], count, total_ingested }", "description": "Query recent buffered entries from LogRingBuffer" },
                { "name": "StreamLogs", "kind": "SERVER_STREAM", "target": "logservice.LogBridge/StreamLogs", "input": "StreamLogsRequest { service_name, min_level }", "output": "stream LogRequest", "description": "Stream incoming logs in real time over HTTP/2" },
                { "name": "GetExchangeContracts", "kind": "RPC", "target": "logservice.LogBridge/GetExchangeContracts", "input": "ContractsRequest {}", "output": "ContractsResponse { service, version, reliable, exhaustive, source, contracts_json }", "description": "Inspect authoritative exchange contracts directly from live log-server" }
            ],
            "schemaFormat": "proto3",
            "schema": LOG_SERVICE_PROTO
        },
        {
            "id": "capnp",
            "type": "Cap'n Proto Contract",
            "tabLabel": "Cap'n Proto Contract",
            "icon": "fa-file-code-o",
            "direction": "inbound",
            "transport": "Zero-Copy Packed Cap'n Proto over SafeSocket TCP",
            "address": tcp_addr,
            "port": config.port,
            "framing": "serialize_packed(LoggerMsg) (schema ID @0xfc867d59c4f2e15c)",
            "auth": "Preceded by unpacked HelloMsg handshake",
            "summary": "Zero-copy wire schema shared across Go, Python, and Rust Universal Logger clients with 12 monotonic severity levels and 16 LoggerMsg text/enum slots.",
            "source": "log-server/src/schema/capnp/logger.capnp (include_str!)",
            "peers": [
                { "service": "universal-logger / fleet daemons", "role": "producer", "description": "Serializes LoggerMsg in Go/Python/Rust with zero string formatting overhead" }
            ],
            "operations": [
                { "name": "Level", "kind": "ENUM", "target": "enum Level (@0xfc867d59c4f2e15c)", "output": "notset(0)..critical(11)", "description": "12 severity levels matching protobuf logservice.Level" },
                { "name": "LoggerMsg", "kind": "STRUCT", "target": "struct LoggerMsg (@0..@15)", "input": "timestamp, hostname, loggerName, module, level, filename, functionName, lineNumber, message, pathName, processId, processName, threadId, threadName, serviceName, stackTrace", "description": "Primary packed wire payload schema for telemetry ingestion" }
            ],
            "schemaFormat": "capnp",
            "schema": LOGGER_CAPNP_SCHEMA
        }
    ])
}

/// Build JSON string of exchange contracts for gRPC GetExchangeContracts.
pub fn build_exchange_contracts_json(config: &Config, ring_capacity: usize) -> String {
    serde_json::to_string(&build_exchange_contracts_value(config, ring_capacity)).unwrap_or_else(|_| "[]".to_string())
}

/// Build canonical OpenMFE ServiceSpec payload for POST /api/v1/register on web-interface.
pub fn build_service_spec_payload(config: &Config, ring_capacity: usize) -> String {
    let version = option_env!("APP_VERSION").unwrap_or(env!("CARGO_PKG_VERSION"));
    let host = display_host(&config.host);
    let contracts = build_exchange_contracts_value(config, ring_capacity);
    let framing = format!("Port {} SafeSocket TCP / Port {} gRPC", config.port, config.grpc_port);

    let payload = json!({
        "name": "log_server",
        "tag": "log-server-inspector",
        "url": "/static/js/log-server-inspector.js",
        "navTitle": "📜 Log Server",
        "layer": "L1",
        "category": "Base Fleet",
        "priority": 15,
        "icon": "fa-file-text-o",
        "requiresAuth": false,
        "capability": "log_server",
        "desc": "Centralized asynchronous log ingestion engine supporting packed Cap'n Proto over SafeSocket TCP and Tonic gRPC streams.",
        "protocol": "Cap'n Proto TCP & gRPC",
        "framing": framing,
        "docLink": "obsidian-brain/06-Microservices/Log-Server-Hub.md",
        "contracts": contracts,
        "dataStructure": {
            "version": version,
            "capability": "log_server",
            "ports": {
                "tcp": config.port.to_string(),
                "grpc": config.grpc_port.to_string(),
                "address": format!("{}:{}", host, config.port)
            },
            "ring_buffer_capacity": ring_capacity
        },
        "sdkLang": "language-rust"
    });

    payload.to_string()
}

/// Non-blocking 30s heartbeat registering log-server's authoritative contracts with web-interface.
pub fn spawn_web_interface_registration(config: Config, ring_capacity: usize) {
    tokio::spawn(async move {
        let web_addr = match microservice_toolbox::config::load_config("standalone") {
            Ok(ac) => match ac.get_listen_addr("web_interface") {
                Ok(addr) if !addr.trim().is_empty() => addr,
                _ => return,
            },
            Err(_) => return,
        };

        let payload = build_service_spec_payload(&config, ring_capacity);
        let mut delay = Duration::from_secs(1);

        loop {
            let request = format!(
                "POST /api/v1/register HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                web_addr,
                payload.len(),
                payload
            );

            let mut registered = false;
            if let Ok(mut stream) = TcpStream::connect(&web_addr).await {
                if stream.write_all(request.as_bytes()).await.is_ok() {
                    let mut resp = Vec::new();
                    let _ = tokio::time::timeout(Duration::from_secs(2), stream.read_to_end(&mut resp)).await;
                    let resp_str = String::from_utf8_lossy(&resp);
                    if resp_str.contains(" 200 ") || resp_str.contains(" 201 ") {
                        registered = true;
                        delay = Duration::from_secs(30);
                    }
                }
            }

            if !registered {
                delay = std::cmp::min(delay * 2, Duration::from_secs(30));
            }
            tokio::time::sleep(delay).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_exchange_contracts_embeds_real_schemas() {
        let cfg = Config::new("log_server", "0.0.0.0", 9020, 9021);
        let json_str = build_exchange_contracts_json(&cfg, 5000);
        let parsed: Value = serde_json::from_str(&json_str).expect("valid contracts json");
        let arr = parsed.as_array().expect("contracts array");
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0]["id"], "safesocket");
        assert_eq!(arr[1]["id"], "proto");
        assert_eq!(arr[2]["id"], "capnp");
        assert!(arr[1]["schema"].as_str().unwrap().contains("GetExchangeContracts"));
        assert!(arr[2]["schema"].as_str().unwrap().contains("struct LoggerMsg"));

        let spec_str = build_service_spec_payload(&cfg, 5000);
        assert!(spec_str.contains("\"contracts\""));
        assert!(spec_str.contains("\"safesocket\""));
    }
}
