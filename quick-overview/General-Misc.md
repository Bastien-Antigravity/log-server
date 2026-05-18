---
tags:
- '#ai/ignore'
- '#zone/3-fleet'
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

Here are the primary diagnostic indicators you may see in the terminal or `logs/_main.log`:

### `[SEQUENCE_GAP_WARNING] Skipping from X to Y due to timeout`
*   **What it means**: Network jitter or an ungraceful client shutdown has lost sequence number `X`.
*   **Action**: The server is functioning perfectly; the built-in **500ms Gap Timer** has bypassed the gap to prevent backpressure from hanging active clients.

### `[BUFFER_FULL_WARNING] Forcing progress to X due to buffer pressure`
*   **What it means**: The sorted in-memory queue has reached its safety capacity limit (1024 packets).
*   **Action**: The server is clearing memory by flushing current logs immediately. Verify client network stability or increase standard file system write speeds.

### `TCP Connection established / Handshake timeout`
*   **What it means**: A client opened a raw TCP socket but failed to send the mandatory Cap'n Proto `HelloMsg` within the 5-second window.
*   **Action**: The server aborted the connection to defend against Slow-Loris attacks. Confirm the client is correctly using the reference Go/Python/Rust `microservice-toolbox` client libraries.
