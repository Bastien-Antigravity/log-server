# Log Server

A high-performance, centralized logging server written in Rust that handles both TCP (Cap'n Proto) and gRPC log messages with ordered file writing and automatic rotation.

## Features

- **Dual Protocol Support**: Accepts log messages via both TCP (Cap'n Proto) and gRPC
- **Ordered Message Writing**: Maintains message sequence integrity using sequence numbers
- **Automatic File Rotation**: Rotates log files based on size with configurable backup count
- **Async Architecture**: Built on Tokio for high-performance concurrent operations
- **Dynamic Batching**: Automatically adjusts batch sizes based on message volume
- **Retry Logic**: Implements retry mechanisms for robust write operations.
- **Integrated gRPC Support**: Runs both TCP and gRPC server components concurrently.

## Architecture

```
log-server/
├── src/
│   ├── config/           # Server configuration (Config struct)
│   ├── core/             # Core business logic
│   │   ├── log_server.rs    # Main orchestrator (LogServer object)
│   │   ├── log_writer.rs    # File/Console output (LogWriter object)
│   │   ├── protocol_handlers.rs # Message processing logic
│   │   └── log_formatter.rs # Logfmt/Console formatting
│   ├── models/           # Data definitions
│   │   └── log_entry.rs     # Central LogEntry model
│   ├── servers/          # Network entry points
│   │   ├── tcp_server.rs    # Cap'n Proto socket server
│   │   └── grpc_server.rs   # Tonic gRPC server
│   ├── transport/        # Low-level communication
│   │   └── safe_socket.rs   # TCP framing and socket management
│   ├── protocols/        # Protocol schemas and generated code
│   │   └── capnp/           # Cap'n Proto generated code
│   ├── utils/
│   │   ├── terminal_ui.rs   # ANSI coloring and terminal helpers
│   │   └── helpers.rs       # IO and string utilities
│   ├── main.rs           # Entry point binary
│   └── lib.rs            # Library entry
├── capnp/
│   └── logger.capnp      # Cap'n Proto schema
├── proto/
│   └── log_service.proto # gRPC proto definition
├── Dockerfile            # Container definition
└── docker-compose.yml    # Multi-container orchestration
```

## Installation

### Prerequisites

- Rust 1.88 or higher
- Protocol Buffers compiler (`protoc`)
- Cap'n Proto compiler (`capnp`)

### Build

```bash
cargo build --release
```

### Docker Usage

You can also run the Log Server using Docker or Docker Compose.

#### Docker Compose (Recommended)

```bash
docker compose up -d
```

#### Docker Build & Run

```bash
# Build the image
docker build -t log-server .

# Run the container
docker run -d -p 9020:9020 -v $(pwd)/logs:/log-server/logs log-server
```

## Usage

### Basic Usage

```bash
# Start server with default settings
./log-server

# Start with custom configuration
./log-server --name MyLogServer --host 0.0.0.0
```

### Command-Line Options

Note: Full configuration uses the standard `microservice-toolbox` address resolution, taking precedence via the `.env` or process environment variables if defined.

| Option          | Default      | Description                            |
|-----------------|--------------|----------------------------------------|
| `--name`        | `log-server` | Server instance name                   |
| `--host`        | `127.0.0.1`  | Host address to bind to                |
| `--grpc_host`   | `127.0.0.1`  | Host address for gRPC to bind to       |

## Message Format

### Cap'n Proto Schema

The TCP server accepts messages in Cap'n Proto format with the following structure:

```capnp
struct LoggerMsg {
  timestamp @0 :Text;
  hostname @1 :Text;
  loggerName @2 :Text;
  module @3 :Text;
  level @4 :Level;
  filename @5 :Text;
  functionName @6 :Text;
  lineNumber @7 :Text;
  message @8 :Text;
  pathName @9 :Text;
  processId @10 :Text;
  processName @11 :Text;
  threadId @12 :Text;
  threadName @13 :Text;
  serviceName @14 :Text;
  stackTrace @15 :Text;
}

enum Level {
  notset @0;
  debug @1;
  stream @2;
  info @3;
  logon @4;
  logout @5;
  trade @6;
  schedule @7;
  report @8;
  warning @9;
  error @10;
  critical @11;
}
```

### gRPC Protocol

The gRPC server uses the `proto/log_service.proto` definition.

```protobuf
service LogService {
  rpc LogMessage(LogRequest) returns (LogResponse);
}

message LogRequest {
  string timestamp = 1;
  string hostname = 2;
  string logger_name = 3;
  string module = 4;
  Level level = 5;
  string filename = 6;
  string function_name = 7;
  string line_number = 8;
  string message = 9;
  string path_name = 10;
  string process_id = 11;
  string process_name = 12;
  string thread_id = 13;
  string thread_name = 14;
  string service_name = 15;
  string stack_trace = 16;
}

enum Level {
  NOTSET = 0;
  DEBUG = 1;
  STREAM = 2;
  INFO = 3;
  LOGON = 4;
  LOGOUT = 5;
  TRADE = 6;
  SCHEDULE = 7;
  REPORT = 8;
  WARNING = 9;
  ERROR = 10;
  CRITICAL = 11;
}
```


### Output Format

Log messages are formatted with fixed-width columns for readability:

```
<timestamp> <hostname> <logger_name> <level> <filename> <function_name> <line_number> <message> <[metadata: extra=extra-data]>
```

Example:
```
2026-04-14T10:30:45.127456789Z myhost       log-server             INFO  tcp_server.rs        run                       48     log-server : TCP server listening on 127.0.0.1:9020 [metadata: mod=log-server]
```

## Configuration

### Writer Configuration

The log writer can be configured in `src/core/log_writer.rs`:

| Setting | Default | Description |
|---------|---------|-------------|
| `initial_batch_size` | `100` | Initial number of messages per write batch |
| `buffer_size` | `1024` | Channel buffer size for incoming messages |
| `max_retries` | `3` | Number of write retry attempts on failure |
| `retry_delay_ms` | `100` | Delay between retries in milliseconds |
| `max_file_bytes` | `1MB` | Maximum file size before rotation |
| `backup_count` | `10` | Number of rotated backup files to keep |

### Log File Location

Log files are stored in the `logs/` directory relative to the executable:

- `logs/_main.log` - Current log file
- `logs/_main.log.0` through `logs/_main.log.9` - Rotated backups

## How It Works

### Message Flow

1. **Reception**: Messages arrive via TCP (Cap'n Proto) or gRPC
2. **Sequencing**: Each message is assigned a unique sequence number
3. **Buffering**: Messages are buffered in a `BTreeMap` ordered by sequence number
4. **Batch Processing**: Messages are written in strict order once a batch is ready or a timeout occurs
5. **File Rotation**: When the current file exceeds the size limit, it is rotated automatically

### Ordered Writing

The server ensures messages are written in strict chronological order based on when they were received:

- Uses sequence numbers to track the exact arrival order
- Buffers out-of-order messages until gaps are filled
- Dynamically adjusts batch size based on buffer depth
- Guarantees no message reordering in the final output file

### TCP Message Framing

For TCP (Cap'n Proto), a simple framing protocol is used:
- **Length Prefix**: 4-byte big-endian unsigned integer (message size)
- **Payload**: Variable-length Cap'n Proto packed message

## API

### TCP (Cap'n Proto) Client

Clients should use the `logger.capnp` schema. Messages must be serialized using Cap'n Proto's "packed" format and prefixed with a 4-byte big-endian length.

### gRPC Client

Clients can use the `proto/log_service.proto` definition. The service name is `LogService` and the method is `LogMessage`.

## Performance Characteristics

- **Async I/O**: Non-blocking operations using Tokio
- **Dynamic Batching**: Adapts to load (10-1000 messages per batch)
- **Connection Pooling**: Handles multiple concurrent clients
- **Buffered Writes**: Minimizes disk I/O operations
- **Retry Logic**: Ensures message durability

## Error Handling

The server handles various error conditions:

- **Connection Errors**: Logs and closes problematic connections
- **Deserialization Errors**: Rejects malformed messages
- **Write Failures**: Retries with exponential backoff
- **Disk Full**: Gracefully handles I/O errors

## Logging

Server operational logs are printed to stdout/stderr:

```text
2026-04-14T10:30:45.123456789Z myhost       log-server             INFO  main.rs              main                      40     log-server : starting log server [metadata: mod=log-server]
2026-04-14T10:30:45.124456789Z myhost       log-server             INFO  main.rs              main                      49     log-server : gRPC server enabled [metadata: mod=log-server]
2026-04-14T10:30:45.125456789Z myhost       log-server             INFO  log_server.rs        run                       52     log-server : starting server components. .  . [metadata: mod=log-server]
2026-04-14T10:30:45.126456789Z myhost       log-server             INFO  log_server.rs        run                       70     log-server : internal logger initialized - writer(s) ready ! [metadata: mod=log-server]
2026-04-14T10:30:45.127456789Z myhost       log-server             INFO  tcp_server.rs        run                       48     log-server : TCP server listening on 127.0.0.1:9020 [metadata: mod=log-server]
2026-04-14T10:30:45.128456789Z myhost       log-server             INFO  grpc_server.rs       run                       63     log-server : gRPC server listening on 127.0.0.1:9021 [metadata: mod=log-server]
2026-04-14T10:30:45.129456789Z myhost       log-server             INFO  log_server.rs        run                       122    log-server : all server components started ! [metadata: mod=log-server]
```

## Development

### Building from Source

```bash
# Debug build
cargo build

# Release build with optimizations
cargo build --release

# Run tests
cargo test

# Check code
cargo clippy
```

### Project Structure

- `src/config/`: Server configuration (Config struct)
- `src/core/`: Core business logic (log_server, log_writer, protocol_handlers)
- `src/models/`: Internal data model (LogEntry)
- `src/servers/`: Network protocol entry points (TCP, gRPC)
- `src/transport/`: Low-level communication logic
- `src/protocols/`: Protocol schemas and generated code
- `src/utils/`: Terminal UI and helper functions

## Testing

The Log Server includes a robust testing infrastructure covering both isolated logic and full system integration.

### What is tested
- **Log Formatting**: Validates fixed-width column alignment, Logfmt metadata serialization, and string truncation.
- **Protocol Handlers**: Verifies correct mapping from Cap'n Proto and gRPC messages to the internal `LogEntry` model.
- **Global Sequencing**: Ensures messages from multiple protocols (TCP/gRPC) share a single, strictly ordered sequence.
- **System Resilience**: Confirms the `LogWriter` handles directory creation and file rotation safely.
- **Microservice Configuration**: Validates environment/CLI argument parsing.
- **Full Integration**: End-to-end testing from TCP/gRPC clients to the physical log file.

### What is not tested yet
- **Extreme Concurrency / Load Testing**: Benchmarks under massive simultaneous client load are not yet automated.
- **Network Resilience / Reconnection**: Drop connections or half-open states edge cases are not fully tested.
- **Malformed Message Resilience**: Deep fuzzing of incoming byte streams for Cap'n Proto / gRPC invalid payloads.

### Test Categories

#### 1. Unit Tests
Located within the source files (e.g., `src/core/log_formatter.rs`), these tests verify internal utilities and data processing logic in isolation.

#### 2. Integration Tests
Located in `tests/integration_tests.rs`, this suite performs a full end-to-end verification:
1. Starts a live `LogServer` instance on test ports.
2. Connects real TCP (Cap'n Proto) and gRPC clients.
3. Sends test traffic and verifies that the output `_main.log` contains correctly ordered and formatted entries.

### How to Run Tests

```bash
# Run all tests (Unit + Integration)
cargo test

# Run only unit tests
cargo test --lib

# Run only integration tests
cargo test --test integration_tests
```

### CI/CD Integration
Every change is automatically validated via GitHub Actions:
- **Linting**: `cargo clippy` enforces best practices.
- **Formatting**: `cargo fmt` ensures style consistency.
- **Automated Testing**: The full test suite must pass before code can be merged.

## Dependencies

Key dependencies:
- `tokio`: Async runtime
- `capnp`: Cap'n Proto serialization
- `tonic`: gRPC framework
- `bytes`: Byte buffer utilities
- `clap`: Command-line argument parsing
- `chrono`: Timestamp handling

