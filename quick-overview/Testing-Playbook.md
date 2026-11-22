---
tags:
- '#ai/ready'
- '#zone/3-fleet'
- '#service/log-server'
- '#type/testing-playbook'
- '#state/active'
microservice: log-server
type: testing-playbook
status: active
---
# 🧪 Log Server: Testing Playbook

This document describes the testing strategy, test suite catalog, and execution playbook for the `log-server` microservice. Our suite ensures absolute reliability under high-throughput burst loads, partial disk write faults, network boundaries, and fail-fast supervision.

---

## 🏗️ The Testing Architecture

The `log-server` testing strategy is split into three cohesive verification layers:

```
[Unit Tests (lib.rs)]   -->  Tests core components in isolation: single-authority FIFO sequencing,
                             transactional disk staging, ftruncate checkpoint rollback, ring buffer
                             FIFO/filtering/broadcast, SafeSocket framing, and protocol handshakes.
[CLI & Address Tests]   -->  Tests in main.rs ensuring robust address resolution, host parsing, and
                             fail-fast behavior when capabilities are absent.
[Integration Pipeline]  -->  Spawns live background TCP & gRPC servers on dynamic ports, streams packed
                             handshakes and gRPC requests, and asserts chronological file output.
```

---

## 📦 Running Tests

Always run tests from the root of the `log-server` repository.

### 1. Run the Entire Test Suite (43 Tests)
Executes all unit and integration tests across library, main, and integration targets:
```bash
cargo test
```

### 2. Run Library Unit Tests Only (35 Tests)
Focuses on internal logic, sequencing, ring buffer, and persistence with zero external port binding:
```bash
cargo test --lib
```

### 3. Run Main Executable Unit Tests Only (3 Tests)
Validates CLI parsing, address resolution, and fail-fast configuration checks:
```bash
cargo test --bin log-server
```

### 4. Run Integration Pipeline Only (1 Test)
Validates the full dual-protocol logging pipeline:
```bash
cargo test --test integration_tests
```

### 5. Run Clippy Static Verification
Ensures strict zero-warning compliance across all targets:
```bash
cargo clippy --all-targets -- -D warnings
```

---

## 🔬 Test Suite Catalog (43 Tests)

### 1. Single-Authority FIFO Sequencing & Concurrency (`src/core/reorder_test.rs`)
* `test_single_authority_fifo_ordering_ignores_caller_sequence_drift`: Asserts that `LogWriter` acts as the single sequence authority, ordering messages strictly by MPSC arrival order and ignoring caller-supplied sequence numbers without synthetic gap delays.
* `test_clean_shutdown_drains_all_pending_messages_without_hanging`: Verifies that dropping channel senders triggers an immediate, clean drain of all pending messages without hanging on periodic flush intervals.
* `test_concurrent_producers_1000_messages_strict_fifo_and_identical_disk_ring_order`: Spawns 10 parallel producer tasks emitting 1,000 total messages. Validates 1-to-1 identical ordering between disk lines and `LogRingBuffer` entries with zero dropped messages.

### 2. Transactional Durability & Checkpoint Rollback (`src/core/transactional_writer.rs`)
* `test_transactional_write_and_flush`: Tests basic contiguous byte-buffer batching, offset tracking, and disk flushing.
* `test_transactional_write_truncation_rollback`: Injects a simulated partial write failure, verifying that Tier 2 POSIX `ftruncate` rolls back the file length to `checkpoint_offset` and retries without leaving corrupted or duplicated prefixes.
* `test_recover_poisoned_handle_restores_checkpoint_boundary`: Tests Tier 3 emergency descriptor re-creation when handle rollback fails, opening a fresh descriptor and enforcing clean record boundaries.
* `test_transactional_rotation_cascading`: Verifies size-based rotation across historical files (`.log.0`, `.log.1`, etc.).
* `test_transactional_writer_sync_on_flush_toggled_on_the_fly`: Validates that toggling `sync_on_flush` on the fly seamlessly transitions from OS page-cache flushes to hardware commits (`file.sync_data()`) and back with zero lost records.

### 3. File Writer Supervision & Fail-Fast Alerting (`src/facade/log_writer.rs`)
* `test_writer_task_fatal_disk_failure_pushes_critical_and_terminates`: Points writer to an unwriteable directory (POSIX `EISDIR`), asserting that `writer_task` records a structured `[DISK_FAILURE]` `CRITICAL` (level 11) log to `LogRingBuffer` and aborts via its `JoinHandle`.
* `test_rotate_files_cascading_and_pruning`: Verifies multi-generation file rotation up to `backup_count` and pruning of older archives.
* `test_rotate_files_zero_backups`: Tests edge case where `backup_count == 0`, ensuring clean deletion without historical archives.

### 4. In-Memory Ring Buffer & Streaming Egress (`src/core/ring_buffer.rs`)
* `test_ring_buffer_capacity_and_fifo`: Asserts bounded 5,000-slot memory usage, monotonic total ingestion counter, and correct FIFO eviction.
* `test_ring_buffer_filters`: Tests filtering by service name and minimum numerical log level.
* `test_ring_buffer_broadcast`: Verifies instant real-time packet delivery to active Tokio broadcast channel subscribers with zero lag.

### 5. SafeSocket Transport & Network Framing (`src/transport/safe_socket.rs`)
* `test_heartbeat_skip`: Validates that zero-length keepalive frames (`0x00000000`) are safely acknowledged without triggering deserialization.
* `test_oversized_message_rejected`: Asserts that payloads exceeding 10MB (`MAX_MESSAGE_SIZE`) are rejected with `InvalidData` to prevent memory exhaustion.
* `test_outgoing_oversized_message_rejected`: Asserts that outgoing frames larger than 10MB are rejected upfront with `InvalidInput` before socket transmission.
* `test_unexpected_eof_mid_message`: Asserts that connections terminated mid-payload return clean `UnexpectedEof` errors rather than hanging.

### 6. Protocol Handlers & Contract Governance (`src/core/contracts.rs`, `protocol_handlers.rs`)
* `test_identify_client_from_valid_handshake`: Verifies client identity extraction (`service@host`) from unpacked Cap'n Proto `HelloMsg`.
* `test_identify_client_from_corrupted_handshake`: Verifies graceful error rejection on malformed handshake frames.
* `test_build_exchange_contracts_embeds_real_schemas`: Asserts that real `.proto` and `.capnp` schema contents are compile-time embedded in ServiceSpec metadata.
* `test_display_host_normalization`: Validates that wildcard hosts (`0.0.0.0`, `::`) normalize to `127.0.0.1` for local service registration.
* `test_resolve_web_interface_addr_from_app_config`: Verifies dynamic discovery of `web_interface` registration endpoint.

### 7. Configuration & Hardware Durability Barrier (`src/config/config.rs`)
* `test_config_new`: Validates initialization of network endpoints with default `fsync: false` OS page cache.
* `test_config_with_grpc_host`: Verifies verbatim host overrides for gRPC listeners.
* `test_config_fsync_toggle_on_the_fly`: Validates that cloning `Config` shares the atomic durability flag and updates propagate immediately across threads.
* `test_parse_fsync_config_defaults`: Tests boolean parsing from `AppConfig` and fallback handling.

### 8. gRPC Server & Model Translation (`src/servers/grpc_server.rs`)
* `test_proto_to_internal_log_entry_roundtrip`: Asserts zero data loss when mapping between Protobuf `LogRequest` and internal `LogEntry`.
* `test_parse_level_names_and_numbers`: Validates bidirectional parsing of log level names and integers.
* `test_resolve_grpc_bind_addr_hostname`, `test_resolve_grpc_bind_addr_ip_literals`, `test_resolve_grpc_bind_addr_invalid_or_empty`: Ensures robust socket address resolution.

### 9. CLI & Service Resolution (`src/main.rs`)
* `test_parse_host_port_preserves_host_verbatim`: Asserts exact host and port preservation from configuration strings.
* `test_resolve_service_addrs_fails_fast_when_missing`: Verifies fail-fast exit when mandatory service definitions are missing.
* `test_resolve_service_addrs_preserves_distinct_grpc_host`: Verifies distinct TCP and gRPC bind host specifications.

### 10. End-to-End Integration Pipeline (`tests/integration_tests.rs`)
* `test_full_log_pipeline`:
  1. Spawns the complete `LogServer` on dynamic background ports (`12920` TCP / `12921` gRPC).
  2. Completes Cap'n Proto `HelloMsg` handshake.
  3. Concurrently ingests packed TCP messages and Tonic gRPC requests.
  4. Shuts down and asserts that all records are persisted to `logs/_main.log` in exact chronological sequence.

---

## 👤 Manual & Operational Testing Tips

1. **Testing Rotation Under Load**:
   Set `max_file_bytes` in `WriterConfig` to `5000` (5KB) in test configurations, blast a stream of logs, and verify that `_main.log.0` through `_main.log.9` rotate cleanly without file descriptor leaks.
2. **Simulating Unrecoverable Disk Failure**:
   Set `base_file_path` to a read-only path or existing directory. Observe that the server pushes `[DISK_FAILURE]` `CRITICAL` to `LogRingBuffer` and exits with code 1.
3. **Observing Live Web Dashboard Streaming**:
   Run `./target/release/log-server` alongside `web-interface` (Port 5000) and open `/mfe/live-logs` to watch live sub-millisecond SSE streaming over the gRPC `StreamLogs` bridge.
