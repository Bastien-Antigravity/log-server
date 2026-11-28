---
tags:
- '#ai/ready'
- '#zone/3-fleet'
- '#service/log-server'
- '#type/features-behavior'
- '#state/active'
- '#ai/ignore'
microservice: log-server
type: features-behavior
status: active
---
# 🧠 Log Server: Features & Behavior

This document describes the runtime behaviors, data contracts, and architectural invariants implemented by the `log-server` microservice to guarantee reliable, ordered log ingestion under real-world network and hardware conditions.

---

## 🗺️ Functional Breakdown

```mermaid
mindmap
  root((log-server Core))
    Ingestion Protocols
      TCP Socket (:9020)
        4-Byte Length Prefix
        Mandatory HelloMsg Handshake
        Cap'n Proto packed serialization
        60s Zombie connection pruning
      gRPC Log Bridge (:9021)
        LogMessage RPC (Ingest)
        GetTailLogs RPC (Snapshot)
        StreamLogs RPC (Live Stream)
        GetExchangeContracts RPC (Schema Discovery)
    Single-Authority Sequencing
      MPSC FIFO arrival channel (2,048 cap)
      Single worker monotonic counter
      Greedy burst draining (try_recv)
      Dynamic batch adaptation (10..1000)
    In-Memory Observability
      LogRingBuffer (5,000 slots)
      Tokio Broadcast Channel (4,096)
      Identical disk-to-RAM commit ordering
      web-interface SSE Bridge
    Transactional Storage
      TransactionalFileWriter
      Tier 1 In-memory staging
      Tier 2 ftruncate checkpoint rollback
      Tier 3 Poisoned descriptor re-creation
      Cascading rotation (10MB limits, 10 backups)
    Fail-Fast Fleet Lifecycle
      In-memory [DISK_FAILURE] level 11 CRITICAL alert
      tokio::select! writer abort
      Process termination with exit code 1
      watchdog-agent crash loop circuit breaker
```

---

## 🎯 Key Behaviors

### 1. Hardened TCP Ingestion & Handshakes
* **Protocol Details**: Runs on Port **`9020`**. Enforces a **4-byte Big-Endian length prefix** on every message payload.
* **Mandatory Handshake**: Upon connection, a client MUST immediately send a standard Cap'n Proto `HelloMsg` specifying its identity (e.g. `service@hostname`).
* **Handshake Timeout (Slow-Loris Guard)**: If no valid handshake is completed within **5 seconds**, the connection is aborted. Skipping handshakes or sending malformed data results in an immediate connection drop.
* **Zombie Pruning**: Connections that remain idle for more than **60 seconds** are automatically pruned to free socket descriptors.

### 2. gRPC Log Bridge (Port `9021`)
* Provides a high-throughput Tonic-based gRPC service (`logservice.LogBridge`) supporting:
  1. `rpc LogMessage(LogRequest) returns (LogResponse)`: Structured message ingestion mapped into internal `LogEntry`.
  2. `rpc GetTailLogs(TailLogsRequest) returns (TailLogsResponse)`: Historical query returning recent buffered logs with optional service and level filters.
  3. `rpc StreamLogs(StreamLogsRequest) returns (stream LogRequest)`: Real-time server-side streaming over HTTP/2.
  4. `rpc GetExchangeContracts(ContractsRequest) returns (ContractsResponse)`: Serves embedded compile-time `.proto` and `.capnp` schema definitions.

### 3. In-Memory Circular Buffer & Real-Time Streaming Egress
* **Ring Buffer (`LogRingBuffer`)**: Retains the last 5,000 log entries in memory using a thread-safe `RwLock<VecDeque<LogEntry>>` with $O(1)$ FIFO wrap-around.
* **Committed Publication**: Entries are pushed to `LogRingBuffer` **only after** transactional disk persistence succeeds, guaranteeing that RAM and `_main.log` maintain identical ordering.
* **Zero-Polling Pipeline**: When `web-interface` connects via `StreamLogs`, logs are pushed down an active HTTP/2 stream with sub-millisecond latency and forwarded to the browser via Server-Sent Events (`/api/v1/system/logs/stream`).

### 4. Single-Authority FIFO Sequencing & Burst Draining
* To eliminate artificial sequence gaps and pre-channel thread race conditions, `LogWriter::writer_task` is the **single serialization and sequence authority**:
  1. Ingestion handlers (TCP, gRPC, internal logger) enqueue `LogPacket` messages into the thread-safe MPSC channel.
  2. `writer_task` owns the monotonic `current_sequence: u64` counter, assigning sequence IDs strictly upon dequeue.
  3. When a packet arrives, `writer_task` opportunistically drains all immediately available packets via `rx.try_recv()` up to `batch_size`.
  4. Dynamic batch size scales between 10 and 1,000 packets based on burst saturation to minimize filesystem syscalls.
  5. Every drained burst is flushed immediately upon `try_recv()` completion, so low-traffic messages are persisted with zero timer delay while high-traffic bursts coalesce into single syscalls.

### 5. Transactional Disk Durability (`TransactionalFileWriter`)
* Protects against partial disk writes, power cuts, and OS write errors across 3 tiers:
  * **Tier 1 (Staging)**: Formats multiple log lines into a contiguous in-memory byte buffer before issuing write syscalls.
  * **Tier 2 (Rollback via Checkpointed `ftruncate`)**: Tracks the exact byte offset prior to writes. On error, truncates the file back to `checkpoint_offset` via `file.set_len(checkpoint_offset)` and retries up to `max_retries` (default 3), dropping corrupt partial prefixes.
  * **Tier 3 (Poisoned Handle Re-creation)**: If truncation fails (e.g. `EIO` or `EROFS`), closes the handle, opens a fresh descriptor, and re-synchronizes the boundary.
  * **Configurable Durability Barrier (`fsync`)**:
    * `capabilities.log_server.fsync: false` (Default): Flushes to the OS page cache via `file.flush()`. Delivers maximum throughput (~200,000+ logs/sec) with zero data loss during process crashes or supervisor restarts (OS kernel persists dirty pages).
    * `capabilities.log_server.fsync: true`: Forces physical NAND flash synchronization on every batch via `file.sync_data()`, guaranteeing immunity even against immediate host power cuts at the cost of disk IOPS limits (~2,000–5,000 logs/sec).
    * **Dynamic On-the-Fly Control**: Wrapped in a shared `Arc<AtomicBool>`, allowing callers or management hooks to toggle hardware commit on the fly (`server.set_sync_on_flush(bool)`) without daemon restarts.
  * **Cascading Rotation**: Rotates files when reaching 10MB, shifting `.log.0` through `.log.9` and pruning historical files beyond `backup_count`.

### 6. Fail-Fast Lifecycle & Fleet Supervision (`watchdog-agent`)
* If disk persistence fails permanently after exhausting retries:
  1. **RAM Alert**: `writer_task` pushes a structured `[DISK_FAILURE]` `CRITICAL` (level 11) record to `LogRingBuffer` so dashboards display the exact failure reason.
  2. **Process Exit**: The server aborts via `tokio::select!` and terminates with exit code 1.
  3. **Fleet Supervision**: `watchdog-agent` detects process termination, applies its **3-crash / 30-second circuit breaker**, suspends auto-restarts into `PAUSED (CrashLoopBackoff)` to prevent disk thrashing, and alerts DevOps via the `tele-remote` Telegram bot.
