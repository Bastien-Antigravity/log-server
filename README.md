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
- '#type/repository'
- '#state/active'
---

# Log Server

A high-performance, centralized logging server written in Rust that handles both TCP (Cap'n Proto) and gRPC log messages with ordered file writing and automatic rotation.

## Features

- **Dual Protocol Support**: Accepts log messages via both TCP (Cap'n Proto) and gRPC concurrently.
- **Single-Authority FIFO Sequencing**: Sequences and batches messages authoritatively inside `LogWriter` in strict MPSC FIFO arrival order, guaranteeing identical ordering between `_main.log` and `LogRingBuffer` without synthetic sequence-gap stalls.
- **Non-Blocking I/O**: Offloads console output to a background task with atomic drop-count telemetry (`get_dropped_console_logs()`) to prevent ingestion stalls.
- **Memory Optimized**: Single-pass pre-allocated `BytesMut` frame reading without intermediate `Vec<u8>` copies and single-buffer `String` log formatting.
- **Automatic File Rotation**: Rotates log files based on size (10MB) with configurable backup count.
- **Append-Only Durability**: Opens log files in **Append Mode**, ensuring data is preserved across server restarts.
- **Colorized Level Support**: Console output highlights log levels with isolated coloring and automatic ANSI reset.
- **Async Architecture**: Built on Tokio for peak multi-threaded throughput.
- **Dynamic Burst Batching**: Automatically drains MPSC bursts via `try_recv()` and scales batch sizes based on ingestion volume.
- **Transactional Disk Recovery**: Uses `TransactionalFileWriter` with in-memory staging, checkpointed `ftruncate` rollback, and verified handle recovery to prevent partial or corrupted disk writes.
- **Real-Time Streaming Egress**: 5,000-slot `LogRingBuffer` broadcasting committed logs to gRPC `StreamLogs` and Web Interface.
- **Fail-Fast Fleet Supervision**: Emits `[DISK_FAILURE]` `CRITICAL` records to RAM and aborts process on exhausted retries, integrating with `watchdog-agent`'s circuit breaker and Telegram alerts.

## Architecture

For a detailed technical deep-dive, please refer to [ARCHITECTURE.md](ARCHITECTURE.md).

The project is structured as follows:

- **Network Layer**: High-performance ingestors for TCP (Cap'n Proto) and gRPC with `JoinSet`-supervised client lifecycles.
- **Core Layer**: Protocol decoding (`hello_msg::Reader`, `logger_msg::Reader`), single-buffer formatter, and `LogRingBuffer`.
- **Persistence Layer**: Single-authority FIFO file writer (`TransactionalFileWriter`) with rotation and non-blocking console output.

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
- `backup_count`: 10 historical log files.
- `buffer_size`: 2048 packets.
- `initial_batch_size`: 100 packets (dynamically scaled 10..1000 based on burst saturation, flushed immediately after draining each available burst).
- `sync_on_flush`: Shared `Arc<AtomicBool>` hardware durability barrier (`fsync`), hot-reloadable via `AppConfig::on_live_conf_update`.

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
