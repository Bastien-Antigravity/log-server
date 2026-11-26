---
tags:
- '#ai/ready'
- '#zone/3-fleet'
- '#service/log-server'
- '#type/operations-manual'
- '#state/active'
- '#ai/ignore'
microservice: log-server
type: operations-manual
status: active
---
# 📂 Log Server: Operations & Reference

This document serves as a practical handbook for deploying, executing, configuring, and troubleshooting the `log-server` microservice in both development and production environments.

---

## 🚀 Operations Runbook

### 1. Build and Run Standalone (Native Rust)

Ensure you have the latest Rust toolchain installed (v1.88+).

**Compile the binary**:
```bash
cargo build --release
```

**Execute the server**:
```bash
cargo run --release
```

By default, the server will bind to `0.0.0.0:9020` for TCP Cap'n Proto connections and `0.0.0.0:9021` for the Tonic gRPC Log Bridge.

---

## 🎛️ Command-Line Configurations

When starting the binary, you can override default values using standard CLI options:

| Flag | Default | Description |
| :--- | :--- | :--- |
| `--name` | `log-server` | Instance identifier for logs and network namespaces. |
| `--host` | `0.0.0.0` | Bind IP address for both TCP and gRPC services. |
| `--port` | `9020` | Dynamic TCP Port to listen for Cap'n Proto packets. |
| `--grpc_host` | `0.0.0.0` | Bind IP address specifically for the Log Bridge (gRPC). |
| `--grpc_port` | `9021` | Dynamic gRPC Port to listen for incoming Tonic bridge calls. |
| `--profile` | `standalone` | Merges configurations from a centralized environment file. |

**Example of Custom Port Configuration**:
```bash
./target/release/log-server --port 9050 --grpc_port 9051
```

---

## 🐳 Containerized Deployments (Docker)

The `log-server` supports dynamic container hosting.

**Build the Docker Image**:
```bash
docker build -t log-server .
```

**Run via Compose**:
The server is integrated into the global compose configuration:
```bash
docker compose up -d log-server
```

> [!IMPORTANT]
> **Security & Reliability (Docker Guard)**:
> If `DOCKER_ENV=true` (or the container file `/.dockerenv` is detected), `microservice-toolbox` will strictly **ignore** CLI-provided binding IP or port overrides. This prevents hardcoded network assumptions from breaking container orchestration networks.

---

## 🩺 Diagnostics & Troubleshooting

Here are the primary diagnostic indicators you may see in the terminal, web dashboard, or `logs/_main.log`:

### `[DISK_FAILURE] Failed writing batch to disk after exhausting retries: <err>`
* **What it means**: The underlying filesystem or storage medium failed write syscalls (e.g. disk full, read-only remount `EROFS`, permission revocation).
* **Action**: `log-server` writes a structured Level 11 (`CRITICAL`) entry to `LogRingBuffer` and exits with code 1. `watchdog-agent` monitors this failure, triggers its 3-crash/30-second circuit breaker, and fires a Telegram alert via `tele-remote`. Check storage quotas and filesystem permissions on the log volume.

### `TCP Connection established / Handshake timeout`
* **What it means**: A client opened a raw TCP socket but failed to send the mandatory Cap'n Proto `HelloMsg` within the 5-second window.
* **Action**: The server aborted the connection to defend against Slow-Loris attacks. Confirm the client is correctly using the reference Go/Python/Rust `microservice-toolbox` client libraries.

### `Unexpected EOF mid-message (expected X bytes)`
* **What it means**: A remote client closed the connection or crashed while in the middle of sending a framed payload.
* **Action**: Logged as an internal warning. The reader safely discards the incomplete payload and resets without affecting other active streams.

### `connection idle for 60s. Pruning zombie.`
* **What it means**: An established client connection remained idle with no heartbeats or log packets for 60 seconds.
* **Action**: Log-server automatically severed the connection to reclaim socket descriptors and prevent resource leaks.
