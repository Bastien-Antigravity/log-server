---
tags:
- '#ai/ignore'
- '#zone/3-fleet'
microservice: log-server
type: testing-playbook
status: active
---
# 🧪 Log Server: Testing Playbook

This document describes the testing strategy and execution playbook for the `log-server` microservice. Our suite ensures absolute reliability under load, gap conditions, and network boundaries.

---

## 🏗️ The Testing Architecture

The `log-server` testing strategy is split into two layers:

```
[Unit Tests]            -->  Tests logic in isolation (formatting, helpers, config, gap recovery)
[Integration Pipeline]  -->  Spawns live background TCP & gRPC servers on dynamic ports, streams packed
                             handshakes and gRPC requests, and asserts chronological file output.
```

---

## 📦 Running Tests

Always run tests from the root of the `log-server` repository.

### 1. Run the Entire Test Suite
This executes all unit and integration tests:
```bash
cargo test
```

### 2. Run Unit Tests Only
Focuses on logical functions with zero network binding:
```bash
cargo test --lib
```

### 3. Run Integration Tests Only
Validates the full dual-protocol logging pipeline:
```bash
cargo test --test integration_tests
```

---

## 🔬 Test Suite Breakdown

### Unit Tests
*   `config::config::tests::test_config_new`: Verifies that server configurations parse environment variables and load defaults safely.
*   `core::log_formatter::tests`:
    *   `test_format_log_message_basic`: Verifies console and file outputs conform to clean fixed-width alignment rules.
    *   `test_format_log_message_with_metadata`: Asserts metadata serialization structures (key=value pairs).
    *   `test_truncation`: Verifies automatic log-message clipping if size thresholds are breached.
*   `core::reorder_test::tests::test_gap_timeout_recovery`: Emulates a 500ms network gap. Asserts that the gap timer expires, triggers recovery, and successfully flushes out-of-order logs.
*   `transport::safe_socket::tests::test_heartbeat_skip`: Validates that heartbeat requests are safely ignored when standard payloads are active.

### Integration Tests
*   `tests/integration_tests.rs`:
    1.  **Bootstrap**: Spawns the complete `LogServer` structure on a dynamic background loop using test ports (`12920` / `12921`).
    2.  **Handshake**: Initializes a standard TCP socket client and fires a valid Cap'n Proto packed `HelloMsg` handshake.
    3.  **Data Generation**: Streams serialized packed TCP log events, then streams independent gRPC requests via Tonic.
    4.  **Verification**: Shuts down the server, parses `logs/_main.log`, and asserts that both channels are successfully integrated in perfect chronological sequence.

---

## 👤 Human Testing Tips

1.  **Testing Rotation Safety**: 
    You can manually force log file rotation in seconds by lowering the byte limit. Set `max_file_bytes` in `WriterConfig` (within `log_writer.rs`) to `1000` (1KB), fire a burst of logs, and verify that `_main.log.0` through `_main.log.9` are successfully generated without dropping a single entry.
2.  **Viewing Local Active Logs**:
    To see the formatted, colorized terminal stream directly, run:
    ```bash
    cargo run --release
    ```
    And use the Go-based client (`universal-logger`) to direct traffic to it.
