# AGENTS.md: log-server

## Service Mission & Architecture Role
`log-server` is a high-performance, asynchronous Rust logging daemon for the Bastien-Antigravity fleet. It accepts length-prefixed framed TCP log streams from microservices over `safe-socket`, processes and formats messages, writes to rotating log files, and broadcasts active log streams to monitoring clients.

- **Language**: Rust (Tokio async runtime)
- **Exposed Capability**: `log_server` (Port: `9020` SafeSocket TCP, Port: `9021` Tonic gRPC `LogBridge`)
- **Framing & Contracts**: 4-byte length prefix + Cap'n Proto payload (`microservice-toolbox` SSoT `logger.capnp` via `microservice_toolbox::contracts::LOGGER_CAPNP_SCHEMA`), gRPC `LogBridge` (`log_service.proto` including `GetExchangeContracts`), and async OpenMFE registry heartbeat to `web-interface` (`src/core/contracts.rs`, `src/facade/log_server.rs`)
- **Configuration Link**: `standalone.yaml -> ../docker-deployment/modes/local/config/native.yaml`

## Key Build & Test Commands
```bash
# Check compilation
cargo check

# Run tests
cargo test

# Build release binary
cargo build --release

# Run
./target/release/log-server
```

## AI Development & Integration Guidelines
1. **Dynamic Port Binding**: Bind port must be read from configuration (`capabilities.log_server.port`), never hardcoded.
2. **SafeSocket Protocol Compliance**: Any protocol change must retain framing parity with `safe-socket` Go and Python clients.
3. **No Blocking Operations in Async Tokio Context**: Avoid synchronous file I/O or sleep calls on Tokio worker threads. Use `tokio::fs` or spawn blocking tasks where needed.
4. **Header Ritual**: All Rust files MUST begin with the Triple-Block header (`ESSENTIAL PROCESS`, `DATA FLOW`, `KEY PARAMETERS`).
5. **Section Dividers**: Use `// -----------------------------------------------------------------------------` between function implementations.
6. **Canonical Platform Schema SSoT & Contract Reflection**: `logger.capnp` resides exclusively in `microservice-toolbox` (`schemas/platform/logger.capnp` and `microservice_toolbox::contracts::LOGGER_CAPNP_SCHEMA`; Go parity: `go/pkg/schemas/logger` `LoggerCapnpSchema`). Use `microservice_toolbox::contracts::extract_capnp_operations(LOGGER_CAPNP_SCHEMA)` (Go parity: `ExtractCapnpOperations`) to populate `web-interface` contracts; never duplicate `logger.capnp` locally or embed via `include_str!("../schema/capnp/logger.capnp")`.
7. **Local-First Rust Workspace Resolution (`.cargo/config.toml`)**: `log-server/Cargo.toml` MUST declare canonical GitHub dependencies (`microservice-toolbox = { git = "https://github.com/Bastien-Antigravity/microservice-toolbox.git", branch = "develop" }`) and NEVER commit local `path = "../..."` dependency or `[patch]` overrides (`RUST_LOCAL_PATH_POLLUTION`). Workspace builds resolve the sibling `microservice-toolbox/rust` crate locally first via root `.cargo/config.toml`, while standalone CI and Docker builds fall back automatically to GitHub.
