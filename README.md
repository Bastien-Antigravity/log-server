# Log Server

A high-performance, centralized logging server written in Rust that handles both TCP (Cap'n Proto) and gRPC log messages with ordered file writing and automatic rotation.

## Features

- **Dual Protocol Support**: Accepts log messages via both TCP (Cap'n Proto) and gRPC
- **Ordered Message Writing**: Maintains message sequence integrity using sequence numbers
- **Automatic File Rotation**: Rotates log files based on size with configurable backup count
- **Async Architecture**: Built on Tokio for high-performance concurrent operations
- **Dynamic Batching**: Automatically adjusts batch sizes based on message volume
- **Retry Logic**: Implements retry mechanisms for robust write operations.
- **Optional gRPC Support**: Runs in TCP-only mode by default; gRPC can be enabled via flag

## Architecture

```
log-server/
├── src/
│   ├── core/           # Core business logic (handlers, writers, orchestration)
│   │   ├── servers.rs  # Main server orchestrator
│   │   ├── handlers.rs # Message processing and formatting
│   │   └── writers.rs  # File writer with ordering and rotation
│   ├── network/        # Network protocol implementations
│   │   ├── tcp_server.rs  # TCP socket server (Cap'n Proto)
│   │   └── grpc_server.rs # gRPC server implementation
│   ├── common/         # Shared utilities and configuration
│   │   ├── config.rs   # Server configuration
│   │   └── safe_socket.rs # Safe TCP socket wrapper with framing
│   ├── logger_capnp/   # Generated Cap'n Proto code
│   ├── utils/
│   │   └── helpers.rs  # Utility functions
│   ├── main.rs         # Entry point
│   └── lib.rs          # Library root
├── capnp/
│   └── logger.capnp    # Cap'n Proto schema
├── proto/
│   └── log_service.proto # gRPC proto definition
├── Dockerfile          # Container definition
└── docker-compose.yml  # Multi-container orchestration
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
./log-server --name MyLogServer --host 0.0.0.0 --port 9020 --grpc_port 9021

# Start with gRPC enabled
./log-server --enable_grpc
```

### Command-Line Options

| Option          | Default      | Description                            |
|-----------------|--------------|----------------------------------------|
| `--name`        | `log-server` | Server instance name                   |
| `--host`        | `127.0.0.1`  | Host address to bind to                |
| `--port`        | `9020`       | TCP server port                        |
| `--grpc_port`   | `9021`       | gRPC server port                       |
| `--enable_grpc` | `false`      | Enable gRPC server (default: TCP only) |

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
<sequence> <timestamp> <hostname> <logger_name> <level> <filename> <function_name> <line_number> <message>
```

Example:
```
0 2025-01-15T10:30:45.123Z  myhost       app_logger      INFO     main.py              process_data              42     Processing started
```

## Configuration

### Writer Configuration

The log writer can be configured in `src/core/writers.rs`:

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

```
LogServer : starting log server
LogServer : starting server components
LogServer : TCP server listening on 127.0.0.1:9020
LogServer : gRPC server listening on 127.0.0.1:9021
LogServer : all server components started
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

- `src/core/`: Core business logic (handlers, writers, orchestration)
- `src/network/`: Network protocol implementations (TCP, gRPC)
- `src/common/`: Shared utilities and configuration
- `src/logger_capnp/`: Generated Cap'n Proto code
- `src/utils/`: Helper functions

## Dependencies

Key dependencies:
- `tokio`: Async runtime
- `capnp`: Cap'n Proto serialization
- `tonic`: gRPC framework
- `bytes`: Byte buffer utilities
- `clap`: Command-line argument parsing
- `chrono`: Timestamp handling

