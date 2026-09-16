# AGENTS.md: log-server

## Service Mission & Architecture Role
`log-server` is a high-performance, asynchronous Rust logging daemon for the Bastien-Antigravity fleet. It accepts length-prefixed framed TCP log streams from microservices over `safe-socket`, processes and formats messages, writes to rotating log files, and broadcasts active log streams to monitoring clients.

- **Language**: Rust (Tokio async runtime)
- **Exposed Capability**: `log_server` (Port: `9020` SafeSocket TCP)
- **Framing**: 4-byte length prefix + payload, 30s heartbeat ping/pong (`safe-socket` standard)
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
