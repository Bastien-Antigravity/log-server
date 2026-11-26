//! ESSENTIAL PROCESS:
//! Constructs exhaustive, compile-time embedded Exchange Contracts directly
//! from log-server schemas (log_service.proto, logger.capnp, and SafeSocket framing)
//! and publishes ServiceSpec metadata to web-interface.
//!
//! DATA FLOW:
//! 1. Active AppConfig + Config -> build_exchange_contracts_value() / resolve_web_interface_addr()
//! 2. spawn_web_interface_registration() -> HTTP POST /api/v1/register -> web-interface
//!
//! KEY PARAMETERS:
//! - config: Runtime TCP/gRPC host and port configuration.
//! - ring_capacity: In-memory LogRingBuffer capacity reported in ServiceSpec.
//! - app_config: Active pre-loaded AppConfig for web_interface service discovery.

use microservice_toolbox::config::loader::AppConfig;
pub use microservice_toolbox::contracts::LOGGER_CAPNP_SCHEMA;
use microservice_toolbox::contracts::extract_capnp_operations;
use microservice_toolbox::spec::{publish, ExchangeContract, ServiceSpec};
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::config::config::Config;

pub const LOG_SERVICE_PROTO: &str = include_str!("../schema/proto/log_service.proto");

//-----------------------------------------------------------------------------------------------

fn display_host(host: &str) -> &str {
    if host.is_empty() || host == "0.0.0.0" || host == "::" {
        "127.0.0.1"
    } else {
        host
    }
}

//-----------------------------------------------------------------------------------------------

/// Build exhaustive list of log-server exchange contracts anchored to runtime config & real schema files.
pub fn build_exchange_contracts_value(config: &Config, ring_capacity: usize) -> Value {
    let capnp_ops = extract_capnp_operations(LOGGER_CAPNP_SCHEMA);
    let host = display_host(&config.host);
    let grpc_host = display_host(&config.grpc_host);
    let tcp_addr = format!("{}:{}", host, config.port);
    let grpc_addr = format!("{}:{}", grpc_host, config.grpc_port);

    let safesocket_spec = [
        "1. Connection Establishment (SafeSocket TCP Handshake / ADR-001):",
        "   - Frame Prefix: 4-byte unsigned big-endian integer length (max 10 MB).",
        "   - Zero-length Frame (0x00000000): Keepalive heartbeat; resets idle timeout without deserialization.",
        "   - Frame #1 (Unpacked Cap'n Proto HelloMsg):",
        "     struct HelloMsg { fromName @0 :Text; fromHost @1 :Text; fromAddress @2 :Text; toAddress @3 :Text; fromPublicIP @4 :Text; }",
        "   - Subsequent Frames (Packed Cap'n Proto LoggerMsg):",
        "     Deserialized via capnp::serialize_packed -> enqueued to MPSC channel -> single-authority FIFO LogWriter batch persistence -> disk & LogRingBuffer.",
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
            "source": "microservice-toolbox/schemas/platform/logger.capnp (SSoT)",
            "peers": [
                { "service": "universal-logger / fleet daemons", "role": "producer", "description": "Serializes LoggerMsg in Go/Python/Rust with zero string formatting overhead" }
            ],
            "operations": capnp_ops,
            "schemaFormat": "capnp",
            "schema": LOGGER_CAPNP_SCHEMA
        }
    ])
}

//-----------------------------------------------------------------------------------------------

/// Build JSON string of exchange contracts for gRPC GetExchangeContracts.
pub fn build_exchange_contracts_json(config: &Config, ring_capacity: usize) -> String {
    serde_json::to_string(&build_exchange_contracts_value(config, ring_capacity)).unwrap_or_else(|_| "[]".to_string())
}

//-----------------------------------------------------------------------------------------------

/// Build canonical OpenMFE ServiceSpec for log-server.
pub fn build_service_spec(config: &Config, ring_capacity: usize) -> ServiceSpec {
    let host = display_host(&config.host);
    let contracts_val = build_exchange_contracts_value(config, ring_capacity);
    let contracts: Vec<ExchangeContract> =
        serde_json::from_value(contracts_val).unwrap_or_default();

    let mut data_structure = HashMap::new();
    data_structure.insert("version".to_string(), Value::String(config.version.clone()));
    data_structure.insert("capability".to_string(), Value::String("log_server".to_string()));
    data_structure.insert(
        "ports".to_string(),
        json!({
            "tcp": config.port.to_string(),
            "grpc": config.grpc_port.to_string(),
            "address": format!("{}:{}", host, config.port)
        }),
    );
    data_structure.insert("ring_buffer_capacity".to_string(), json!(ring_capacity));

    ServiceSpec {
        name: "log_server".to_string(),
        tag: "log-server-inspector".to_string(),
        url: "/static/js/log-server-inspector.js".to_string(),
        nav_title: "📜 Log Server".to_string(),
        layer: "L1".to_string(),
        category: "Base Fleet".to_string(),
        priority: 15,
        icon: "fa-file-text-o".to_string(),
        requires_auth: false,
        capability: "log_server".to_string(),
        description: "Centralized asynchronous log ingestion engine supporting packed Cap'n Proto over SafeSocket TCP and Tonic gRPC streams.".to_string(),
        protocol: "Cap'n Proto TCP & gRPC".to_string(),
        framing: format!("Port {} SafeSocket TCP / Port {} gRPC", config.port, config.grpc_port),
        doc_link: "obsidian-brain/06-Microservices/Log-Server-Hub.md".to_string(),
        contracts,
        data_structure,
        sdk_lang: "language-rust".to_string(),
        ..ServiceSpec::default()
    }
}

/// Build canonical OpenMFE ServiceSpec payload JSON string for log-server.
pub fn build_service_spec_payload(config: &Config, ring_capacity: usize) -> String {
    serde_json::to_string(&build_service_spec(config, ring_capacity))
        .unwrap_or_else(|_| "{}".to_string())
}

//-----------------------------------------------------------------------------------------------

/// Resolve the `web_interface` registration address from the active `AppConfig`.
pub fn resolve_web_interface_addr(app_config: &AppConfig) -> Option<String> {
    let addr = app_config.get_listen_addr("web_interface").ok()?;
    let trimmed = addr.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some((h, p)) = trimmed.rsplit_once(':') {
        Some(format!("{}:{}", display_host(h), p))
    } else {
        Some(trimmed.to_string())
    }
}

//-----------------------------------------------------------------------------------------------

/// Non-blocking registration publishing log-server's ServiceSpec & ExchangeContracts via microservice-toolbox.
pub fn spawn_web_interface_registration(
    config: Config,
    ring_capacity: usize,
    app_config: Option<&AppConfig>,
) {
    let ac = match app_config {
        Some(ac) if resolve_web_interface_addr(ac).is_some() => ac,
        _ => return,
    };

    let spec = build_service_spec(&config, ring_capacity);
    let _ = publish(ac, None, "log_server", spec);
}

//-----------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_build_exchange_contracts_embeds_real_schemas() {
        let cfg = Config::new("log_server", "0.0.0.0", 19020, 19021).with_grpc_host("log-server");
        let json_str = build_exchange_contracts_json(&cfg, 5000);
        let parsed: Value = serde_json::from_str(&json_str).expect("valid contracts json");
        let arr = parsed.as_array().expect("contracts array");
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0]["id"], "safesocket");
        assert_eq!(arr[0]["address"], "127.0.0.1:19020");
        assert_eq!(arr[1]["id"], "proto");
        assert_eq!(arr[1]["address"], "log-server:19021");
        assert_eq!(arr[2]["id"], "capnp");
        assert_eq!(
            arr[2]["source"],
            "microservice-toolbox/schemas/platform/logger.capnp (SSoT)"
        );
        let capnp_ops = arr[2]["operations"]
            .as_array()
            .expect("capnp operations array");
        assert_eq!(capnp_ops.len(), 2);
        assert_eq!(capnp_ops[0]["name"], "Level");
        assert_eq!(capnp_ops[0]["kind"], "ENUM");
        assert_eq!(capnp_ops[1]["name"], "LoggerMsg");
        assert_eq!(capnp_ops[1]["kind"], "STRUCT");
        assert!(arr[1]["schema"].as_str().unwrap().contains("GetExchangeContracts"));
        assert!(arr[2]["schema"].as_str().unwrap().contains("struct LoggerMsg"));

        let spec_str = build_service_spec_payload(&cfg, 5000);
        assert!(spec_str.contains("\"contracts\""));
        assert!(spec_str.contains("\"safesocket\""));
    }

    #[test]
    fn test_display_host_normalization() {
        assert_eq!(display_host("0.0.0.0"), "127.0.0.1");
        assert_eq!(display_host("::"), "127.0.0.1");
        assert_eq!(display_host(""), "127.0.0.1");
        assert_eq!(display_host("192.168.1.50"), "192.168.1.50");
        assert_eq!(display_host("example.local"), "example.local");
    }

    #[test]
    fn test_resolve_web_interface_addr_from_app_config() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = std::env::temp_dir().join(format!("log_server_contracts_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let empty_cfg_path = dir.join("empty.yaml");
        std::fs::write(&empty_cfg_path, "{}\n").expect("write empty temp config");

        let prev_cfg_path = std::env::var_os("CONFIG_PATH");
        std::env::set_var("CONFIG_PATH", &empty_cfg_path);
        let empty_ac = microservice_toolbox::config::load_config("empty").expect("load empty config");
        assert_eq!(resolve_web_interface_addr(&empty_ac), None);

        let cfg_path = dir.join("production.yaml");
        std::fs::write(
            &cfg_path,
            "capabilities:\n  web_interface:\n    ip: web-interface\n    port: '15000'\n",
        )
        .expect("write temp config");

        std::env::set_var("CONFIG_PATH", &cfg_path);
        let ac = microservice_toolbox::config::load_config("production").expect("load temp config");
        match prev_cfg_path {
            Some(v) => std::env::set_var("CONFIG_PATH", v),
            None => std::env::remove_var("CONFIG_PATH"),
        }
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(
            resolve_web_interface_addr(&ac),
            Some("web-interface:15000".to_string())
        );
    }
}
