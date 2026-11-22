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

## 🛡️ Hardening (Spec-First)
- [x] **Ingestion Gap Protection**: Implement a 500ms timeout for missing sequence IDs in `LogWriter` (FEAT-002).
- [x] **Protocol Hardening**: Enforce 4-byte BE framing and Mandatory Handshake (FEAT-006).
- [x] **Semantic Refactor**: Rename gRPC ingestion to "Log Bridge" to distinguish from Control Ports.
- [x] **Rotation Safety**: Verify rotation doesn't drop messages during file switch (FEAT-004) - *Verified: single-threaded async writer task with backpressure is safe.*
- [x] **Real-Time Log Streaming & Ring Buffer (FEAT-007)**: In-memory circular buffer (LogRingBuffer, 5,000 slots) and gRPC server-side streaming (StreamLogs / GetTailLogs) consumed directly by web-interface for live console observability.

## 🏗️ Technical Debt
- [x] Refactor GRPC/TCP error messages for clarity - *Verified: structured logging provides full context and status.*
- [x] Audit microservice-toolbox CLI parameters for parity - *Verified: 100% matched parity.*

## 🔍 Audit Findings (2026-11-21)

### 🔴 High

- [ ] **Global sequence ordering mixes independent ingestion sources**: `sequence_counter` is shared by TCP, gRPC, and internal logger packets.
  - This is an ordering-integrity concern, not a delivery-availability concern. The intended contract is best-effort chronological ordering, with gap recovery and overflow protection.
  - Document that global monotonic ordering is not a strict cross-source transaction guarantee.
  - Review whether the reorder buffer should be source-aware or whether sequence numbers should remain explicitly local to a single ingestion stream.
  - **Affected areas**: `src/servers/tcp_server.rs`, `src/servers/grpc_server.rs`, `src/utils/terminal_ui.rs`, `src/facade/log_writer.rs`.

- [ ] **Implement explicit graceful shutdown**: Supervise and stop TCP, gRPC, writer, heartbeat, and OpenMFE tasks together so final buffered logs are drained and sockets/tasks are released.
  - **Affected area**: `src/facade/log_server.rs`.

- [ ] **Prevent internal logger task amplification**: Replace full-channel detached send-task fan-out with bounded backpressure, coalescing, or explicit drop accounting to avoid unbounded waiting tasks during log flooding.
  - **Affected area**: `src/utils/terminal_ui.rs`.

- [ ] **Fix IPv6 bind address formatting**: TCP currently formats `host:port` verbatim and will produce an invalid unbracketed address for IPv6 hosts such as `::1`; use one canonical bind-target formatter for TCP and gRPC.
  - **Affected area**: `src/servers/tcp_server.rs`, `src/main.rs`.

- [ ] **Bound outgoing SafeSocket frames**: Reject payload sizes that cannot fit in the protocol's `u32` length field before casting and sending, in addition to retaining reader-side 10 MB protection.
  - **Affected area**: `src/transport/safe_socket.rs`.

- [ ] **Add explicit disk durability barriers**: Determine whether `sync_all`/`fsync` is required at file or batch boundaries and define the crash-consistency performance trade-off.
  - **Affected area**: `src/core/transactional_writer.rs`.

### 🟠 Medium

- [ ] **Replace silent version fallbacks**: Remove `CARGO_PKG_VERSION` and ad-hoc `VERSION.txt` fallback behavior; use strict `AppConfig`/ecosystem version access and fail fast when version metadata is unavailable.
  - **Affected areas**: `build.rs`, `src/main.rs`, `src/core/contracts.rs`, `src/servers/grpc_server.rs`.

- [ ] **Replace the handshake schema-layout hack**: Use the actual SafeSocket handshake schema or a canonical Rust representation instead of decoding `HelloMsg` as `LoggerMsg`.
  - **Affected area**: `src/core/protocol_handlers.rs`.

- [ ] **Use canonical OpenMFE/ServiceSpec tooling**: Replace direct HTTP registration construction and fixed retry behavior with `PublishWithLifecycle`, `AppConfig` address resolution, infinite retry, and randomized jitter.
  - **Affected area**: `src/core/contracts.rs`.

- [ ] **Remove silent wildcard-host fallbacks**: Avoid rewriting `0.0.0.0`, `::`, or empty hosts to `127.0.0.1` for contracts and published addresses; distinguish bind addresses from advertised/display addresses.
  - **Affected area**: `src/core/contracts.rs`.

- [ ] **Preserve ring-stream ordering semantics**: Separate ingestion state from ordered disk state or ensure the ring buffer and gRPC stream expose a consistently defined ordering contract.
  - **Affected areas**: `src/facade/log_writer.rs`, `src/core/ring_buffer.rs`, `src/servers/grpc_server.rs`.

- [ ] **Make ring-buffer locking and broadcast behavior explicit**: Remove or handle poisoned-lock silent skips and document/cohesively handle broadcast lag without inconsistent storage behavior.
  - **Affected area**: `src/core/ring_buffer.rs`.

- [ ] **Prevent console output loss**: Replace `try_send` queue drops with bounded backpressure or visible drop metrics for the console worker.
  - **Affected area**: `src/facade/log_writer.rs`.

- [ ] **Harden file rotation**: Add crash-consistency and partial-failure handling for cascading renames rather than relying on sequential best-effort renames.
  - **Affected area**: `src/core/transactional_writer.rs`.

- [ ] **Reduce blocking operations in async paths**: Move startup and disk folder operations to appropriate async or blocking-task boundaries and eliminate direct daemon `println!`/`eprintln!` in favor of the canonical logger facade.

### 🧪 Missing Adversarial Coverage

- [ ] IPv6 bind and advertised-address coverage.
- [ ] Partial writes and interrupted disk writes.
- [ ] Duplicate sequence numbers.
- [ ] Sequence-number overflow and wraparound behavior.
- [ ] Writer/channel saturation and sustained log flooding.
- [ ] Slow or lagged gRPC consumers.
- [ ] SafeSocket 10 MB boundary payloads and oversized payload rejection.
- [ ] Writer task panic and restart/fail-fast behavior.
- [ ] Disk-full conditions and quota exhaustion.
- [ ] Graceful shutdown with buffered TCP, gRPC, internal, and writer data.
- [ ] Malformed and padded Cap'n Proto frames.
- [ ] Mid-handshake socket closure and handshake timeout behavior.
- [ ] Zero-length frame sequences under active read/write deadlines.
- [ ] Rotation crash at every rename step and recovery after partial rotation.
- [ ] Ring-buffer lock-poison and broadcast-lag edge cases.

### 🧪 Test Quality Improvements

- [ ] Replace test `return`/skip behavior for denied loopback, server readiness, and startup failures with explicit environment-gated skips or hard failures as appropriate.
- [ ] Add an explicit strict-Clippy gate for all targets, initially resolving `collapsible_if` in `src/main.rs:165`.
- [ ] Add end-to-end tests for sequence-gap, out-of-order, duplicate, late-arriving, and overflow scenarios.
- [ ] Document the expected best-effort ordering SLA and the accepted worst-case behavior under buffer pressure or prolonged gaps.