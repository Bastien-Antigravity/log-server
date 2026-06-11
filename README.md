---
microservice: log-server
type: repository
status: active
language: rust
tags:
- '#service/log-server'
- '#domain/observability'
- '#domain/networking'
- '#zone/3-fleet'
---

# Log Server

A high-performance, centralized logging server written in Rust that handles both TCP (Cap'n Proto) and gRPC log messages with ordered file writing and automatic rotation.

## Features

- **Dual Protocol Support**: Accepts log messages via both TCP (Cap'n Proto) and gRPC concurrently.
- **Ordered Message Writing**: Maintains strict sequence integrity using a `BTreeMap` reorder buffer.
- **Non-Blocking I/O**: Offloads console output to a background task to prevent ingestion stalls.
- **Memory Optimized**: Reusable stack-based buffers for chunked network reads to minimize heap churn.
- **Automatic File Rotation**: Rotates log files based on size (10MB) with configurable backup count.
- **Append-Only Durability**: Opens log files in **Append Mode**, ensuring data is preserved across server restarts.
- **Colorized Level Support**: Console output highlights log levels with isolated coloring and automatic ANSI reset.
- **Async Architecture**: Built on Tokio for peak multi-threaded throughput.
- **Dynamic Batching**: Automatically adjusts batch sizes based on current ingestion volume.
- **Data Loss Audit**: Automatically logs `[SEQUENCE_GAP]` and `[BUFFER_PRESSURE]` entries if data is lost or delayed.

## Architecture

For a detailed technical deep-dive, please refer to [ARCHITECTURE.md](ARCHITECTURE.md).

The project is structured as follows:

- **Network Layer**: High-performance ingestors for TCP (Cap'n Proto) and gRPC.
- **Core Core**: Ordering sequencer and reorder buffer.
- **Persistence Layer**: Ordered file writer with rotation and non-blocking console output.

## Installation

### Prerequisites

- Rust 1.88 or higher
- Protocol Buffers compiler (`protoc`)
- Cap'n Proto compiler (`capnp`)

### Build

```bash
cargo build --release
```

## Usage

### TCP Protocol (Hardened)

The TCP server (Port 9020) enforces a strict protocol:
1.  **Framing**: 4-byte Big-Endian length prefix.
2.  **Handshake**: Mandatory `HelloMsg` identity exchange on connection.
3.  **Security**: 60-second read timeout to prune zombie connections.

### Log Bridge (gRPC)

The Log Bridge (Port 9021) provides a gRPC gateway for structured logging via `LogService`.

## Configuration

Writer settings in `src/facade/log_writer.rs`:
- `max_file_bytes`: 10MB default.
- `gap_timeout_ms`: 500ms before triggering a synthetic gap entry.
- `buffer_size`: 2048 packets.

## Project Structure

- `src/facade/`: Main orchestrators (`LogServer`, `LogWriter`).
- `src/servers/`: Protocol ingestors (`TcpServer`, `GrpcServer`).
- `src/transport/`: Optimized networking (`SafeSocket`).
- `src/core/`: Protocol handlers and ordering logic.
- `src/schema/`: Cap'n Proto and Protobuf definitions.

## 🛡️ Testing & Verification
```bash
cargo test
```
All components are verified via the **Spec-First Protocol**.
