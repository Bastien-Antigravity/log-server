# Testing Infrastructure

This document provides a detailed overview of the testing strategy, tools, and procedures for the Log Server project.

## Overview

The Log Server employs a dual-layered testing strategy to ensure both micro-level logic correctness and macro-level system stability. All tests are designed to be environment-agnostic as much as possible, though some integration tests require local networking capabilities.

---

## 1. Unit Testing

Unit tests focus on isolated functional components without external dependencies like the file system or network.

### Coverage
- **`src/core/log_formatter.rs`**: 
    - Validates the 125-character fixed-column header alignment.
    - Verifies that Logfmt metadata (e.g., `[metadata: key=value]`) is correctly serialized.
    - Teasts string truncation logic for long function or module names.
- **`src/utils/helpers.rs`**:
    - Ensures UTC timestamps follow the `%Y-%m-%dT%H:%M:%S%.3fZ` format.
    - Validates sequence number parsing from raw strings.
- **`src/config/config.rs`**:
    - Verifies default values and correct mapping of environment/CLI arguments to the `Config` struct.

### Execution
```bash
# Run only unit tests
cargo test --lib
```

---

## 2. Integration Testing

Integration tests verify the full "Network-to-Disk" logging pipeline. They reside in the `tests/` directory.

### The Integration Pipeline (`tests/integration_tests.rs`)
The primary integration test (`test_full_log_pipeline`) performs the following steps:

1. **Environment Setup**: Dynamically identifies the executable's directory to find the appropriate `logs/` folder.
2. **Server Spawning**: Spawns a live `LogServer` instance in a background `tokio` task.
3. **Multi-Protocol Traffic**:
    - **TCP Client**: Serializes a `LoggerMsg` using **Cap'n Proto**, adds the 4-byte framing header, and sends it via `TcpStream`.
    - **gRPC Client**: Uses a **Tonic** client to send a `LogRequest` to the gRPC endpoint.
4. **Synchronization**: Implements a retry-loop with a 15-second timeout to allow the async `LogWriter` to flush batches to disk.
5. **Validation**: Reads the resulting `_main.log` and asserts that both messages exist and follow the expected format.

### Execution
```bash
# Run the integration suite
cargo test --test integration_tests

# Run with console output (useful for debugging server logs)
cargo test --test integration_tests -- --nocapture
```

---

## 3. CI/CD & Static Analysis

Every pull request and push to `develop`/`main` triggers the GitHub Actions pipeline.

### Quality Gates
- **Formatting (`cargo fmt`)**: Enforces strict adherence to the Rust Style Guide.
- **Linting (`cargo clippy`)**: Prevents common pitfalls, performance issues, and non-idiomatic code. **CI is configured to fail on any warning.**
- **Automated Execution**: Both unit and integration tests must pass in a clean Linux environment (`ubuntu-latest`).

### Caching
We use `Swatinem/rust-cache` to store the `target/` directory. This reduces average CI runtimes from ~5 minutes to **under 90 seconds**.

---

## 4. Cross-Platform Considerations

### Windows Development
- **Protoc/Capnp**: Ensure `protoc` and `capnp` are in your Windows `%PATH%`.
- **RUSTFLAGS**: To match CI strictness locally on Windows (PowerShell):
  ```powershell
  $env:RUSTFLAGS="-Dwarnings"; cargo check
  ```

### Linux (Debian/Ubuntu)
- Required system packages: `protobuf-compiler` and `capnproto`.
- Use `cargo test` as normal.

---

## 5. Adding New Tests

When adding features, follow these guidelines:
1. **Isolated Logic?** Put it in a `#[cfg(test)] mod tests` block at the bottom of the relevant `.rs` file.
2. **System Flow?** Add a new `#[tokio::test]` to `tests/integration_tests.rs`.
3. **Ordering Matters**: If testing ordering, remember that `LogWriter` uses dynamic batching. You may need to send multiple messages to trigger a flush or wait for the internal flush timeout.
