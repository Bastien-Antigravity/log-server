//! ESSENTIAL PROCESS:
//! Network communication servers module. Exposes TCP socket (Cap'n Proto)
//! and gRPC bridge (Tonic/HTTP2) services.
//!
//! DATA FLOW:
//! 1. Inbound network connections -> tcp_server / grpc_server
//! 2. Ingested log messages -> writer_tx
//!
//! KEY PARAMETERS:
//! - grpc_server: Port 9021 gRPC ingestion and HTTP/2 real-time streaming.
//! - tcp_server: Port 9020 Cap'n Proto SafeSocket ingestion.

pub mod grpc_server;
pub mod tcp_server;
