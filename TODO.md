---
microservice: log-server
type: todo
status: active
tags:
- '#service/log-server'
- '#type/todo'
- '#state/active'
- '#zone/1-core'
---

# Log Server TODO

## 🎯 Active Architectural Tasks

### 🔴 High Priority
- [ ] **Dedicated Cap'n Proto Handshake Schema**: Replace the `@0:fromName` / `@1:fromHost` field layout aliasing in `protocol_handlers.rs` with a dedicated `HelloMsg` Cap'n Proto schema reader.
  - *Affected area*: `src/core/protocol_handlers.rs`.

### 🟠 Medium Priority
- [ ] **`microservice-toolbox` Config & SafeSocket Convergence**: Evaluate delegating low-level framing and transport to `microservice-toolbox::transport::SafeSocket` (or shared Rust SDK equivalent) to guarantee cross-language protocol parity, and wire `AppConfig::on_live_conf_update` directly to `Config::set_sync_on_flush` for distributed fleet-wide live configuration reloads.
  - *Affected area*: `src/transport/safe_socket.rs`, `src/config/config.rs`, `src/facade/log_server.rs`.

---

## 🧪 Sandbox-Testing Scenarios to Implement

The following end-to-end integration and resilience test scenarios are targeted for implementation in `sandbox-testing/02-Scenarios/go` and `python` to validate `log-server` in multi-container Docker fleet environments:

### 1. `SCEN-LS-001: Transactional Disk Write Failure & Fail-Fast Verification`
* **Target File**: `sandbox-testing/02-Scenarios/go/log_server_disk_failure_test.go`
* **Goal**: Verify that when the log volume becomes read-only or the disk fills, `log-server` fails fast and does not become a zombie process.
* **Steps**:
  1. Boot `log-server` in Docker with a mounted log volume.
  2. Remount the log volume read-only (`mount -o remount,ro /logs`).
  3. Send log bursts via TCP and gRPC.
  4. Assert that `log-server` terminates with exit code 1 within 5 seconds.
  5. Assert that TCP (9020) and gRPC (9021) sockets are cleanly severed rather than accepting unpersisted data.

### 2. `SCEN-LS-002: Watchdog-Agent Supervision & Crash-Loop Circuit Breaker Integration`
* **Target File**: `sandbox-testing/02-Scenarios/go/log_server_watchdog_circuit_breaker_test.go`
* **Goal**: Validate ecosystem-level supervision between `watchdog-agent` and `log-server`.
* **Steps**:
  1. Launch `log-server` supervised by `watchdog-agent` (`--supervise`).
  2. Cause 3 consecutive process aborts within 30 seconds (via simulated fatal I/O).
  3. Verify `watchdog-agent` catches process death, logs `🚨 CRASH LOOP DETECTED`, and pauses restarts into `PAUSED (CrashLoopBackoff)`.
  4. Intercept the simulated `tele-remote` Telegram notification.
  5. Send `POST /api/v1/watchdog/resume {"name": "log_server"}` and verify recovery.

### 3. `SCEN-LS-003: Multi-Producer Concurrent Blast & Identical Disk/Ring-Buffer Ordering Parity`
* **Target File**: `sandbox-testing/02-Scenarios/go/log_server_parity_test.go`
* **Goal**: Verify zero sequence gaps, zero dropped messages, and 100% identical ordering between physical `_main.log` and the in-memory `LogRingBuffer` / `StreamLogs` gRPC stream.
* **Steps**:
  1. Concurrently spawn 20 client workers (10 SafeSocket TCP + 10 gRPC) sending 500 messages each (10,000 total).
  2. Subscribe to `StreamLogs` via gRPC concurrently.
  3. Compare received `StreamLogs` stream with the contents of `_main.log`.
  4. Assert line-by-line ordering parity and zero dropped messages.

### 4. `SCEN-LS-004: Cascading File Rotation & Pruning Under Continuous High-Throughput Burst`
* **Target File**: `sandbox-testing/02-Scenarios/go/log_server_rotation_test.go`
* **Goal**: Verify high-speed size-based rotation across multiple file generations without descriptor leaks.
* **Steps**:
  1. Configure `max_file_bytes = 1MB` and `backup_count = 5`.
  2. Stream 10MB of structured log data.
  3. Verify that `_main.log`, `_main.log.0`, `_main.log.1` ... `_main.log.4` exist.
  4. Verify that historical files older than `backup_count` are pruned.
  5. Verify no open file descriptor leaks via `lsof` / container metrics.

### 5. `SCEN-LS-005: Premature Socket Severance, UnexpectedEof, and Oversized Frame Rejection`
* **Target File**: `sandbox-testing/02-Scenarios/go/log_server_fuzz_framing_test.go`
* **Goal**: Validate transport robustness against corrupted frames, slow-loris handshakes, and premature socket closures.
* **Steps**:
  1. Send valid 4-byte BE length prefix for 64KB, but close the socket after sending only 128 bytes.
  2. Assert server logs `UnexpectedEof` mid-message and reclaims resources cleanly.
  3. Send frame with length prefix `20MB` (> 10MB `MAX_MESSAGE_SIZE`).
  4. Assert server immediately drops connection with `InvalidData` without crashing or allocating excessive heap.

### 6. `SCEN-LS-006: OpenMFE Auto-Registration & Dynamic Contract Schema Validation`
* **Target File**: `sandbox-testing/02-Scenarios/go/log_server_mfe_registration_test.go`
* **Goal**: Verify OpenMFE contract registration against `web-interface`.
* **Steps**:
  1. Boot mock `web-interface` HTTP endpoint on port 5000.
  2. Launch `log-server`.
  3. Assert `POST /api/v1/register` is received with valid ServiceSpec, embedding real schemas for `log_service.proto` and `logger.capnp`.
  4. Call gRPC `LogBridge.GetExchangeContracts` and verify schema equality with the registration payload.

---

## 🏛️ Completed Architecture Milestones (Traceability)

- [x] **Single-Authority FIFO Sequencing**: Monotonic sequence assignment upon dequeue, greedy burst draining (`rx.try_recv()`), and identical disk-to-ring commit order (`src/facade/log_writer.rs`).
- [x] **Transactional Disk Persistence & Checkpoint Rollback**: Tier 1 in-memory staging, Tier 2 `ftruncate` rollback on partial writes, and Tier 3 handle recovery (`src/core/transactional_writer.rs`).
- [x] **Fail-Fast Lifecycle Integration**: `writer_handle` supervised in `tokio::select!`, `[DISK_FAILURE]` `CRITICAL` pushed to `LogRingBuffer`, and non-zero exit code 1 (`src/facade/log_server.rs`).
- [x] **In-Memory Ring Buffer & gRPC StreamLogs Bridge**: 5,000-slot `LogRingBuffer` and HTTP/2 `StreamLogs` consumed by `web-interface` (`src/core/ring_buffer.rs`, `src/servers/grpc_server.rs`).
- [x] **Hardened SafeSocket Framing**: 4-byte BE length framing, 10MB limits, 5s Slow-Loris guard, 60s zombie pruning, and keepalive heartbeat handling (`src/transport/safe_socket.rs`).
- [x] **OpenMFE Exchange Contracts**: Compile-time embedded `.proto` and `.capnp` schemas served via `GetExchangeContracts` and registered with `web-interface` (`src/core/contracts.rs`).
- [x] **Strict Clippy Compliance**: Zero compiler warnings across all targets (`cargo clippy --all-targets -- -D warnings`).


---

## 🏛️ Completed Architecture Milestones (Traceability)

- [x] **Single-Authority FIFO Sequencing**: Monotonic sequence assignment upon dequeue, greedy burst draining (`rx.try_recv()`), and identical disk-to-ring commit order (`src/facade/log_writer.rs`).
- [x] **Transactional Disk Persistence & Checkpoint Rollback**: Tier 1 in-memory staging, Tier 2 `ftruncate` rollback on partial writes, and Tier 3 handle recovery (`src/core/transactional_writer.rs`).
- [x] **Fail-Fast Lifecycle Integration**: `writer_handle` supervised in `tokio::select!`, `[DISK_FAILURE]` `CRITICAL` pushed to `LogRingBuffer`, and non-zero exit code 1 (`src/facade/log_server.rs`).
- [x] **In-Memory Ring Buffer & gRPC StreamLogs Bridge**: 5,000-slot `LogRingBuffer` and HTTP/2 `StreamLogs` consumed by `web-interface` (`src/core/ring_buffer.rs`, `src/servers/grpc_server.rs`).
- [x] **Hardened SafeSocket Framing**: 4-byte BE length framing, 10MB limits, 5s Slow-Loris guard, 60s zombie pruning, and keepalive heartbeat handling (`src/transport/safe_socket.rs`).
- [x] **OpenMFE Exchange Contracts**: Compile-time embedded `.proto` and `.capnp` schemas served via `GetExchangeContracts` and registered with `web-interface` (`src/core/contracts.rs`).
- [x] **Strict Clippy Compliance**: Zero compiler warnings across all targets (`cargo clippy --all-targets -- -D warnings`).