# 🧬 Project DNA: log-server

## 🎯 High-Level Intent (BDD)
- **Goal**: Provide a centralized, high-performance sink for all ecosystem logs, supporting both TCP (SafeSocket) and gRPC transports.
- **Key Pattern**: **Zero-Copy Sink** (using `mmap` for disk writes) and **Cap'n Proto Deserialization** for high-throughput stream processing.
- **Behavioral Source of Truth**: [[business-bdd-brain/02-Behavior-Specs/log-server]]
- **Spec Gate**: [HARDENED] No implementation without an `approved` spec in the folder above.

## 🛠️ Role Specifics
- **Architect**: 
    - Ensure asynchronous, non-blocking disk I/O to prevent logging from slowing down clients.
    - Maintain gRPC and TCP protocol parity for incoming log streams.
- **QA**: 
    - Stress test with 10k+ concurrent connections.
    - Verify file rotation and disk-full scenarios.
- **Developer**:
    - Follow strict Rust memory safety patterns (avoid `unsafe` unless justified in `mmap` layers).

## 🚦 Lifecycle & Versioning
- **Primary Branch**: `develop`
- **Protected Branches**: `main`, `master`
- **Versioning Strategy**: Semantic Versioning (vX.Y.Z).
- **Version Source of Truth**: `VERSION.txt` (Must be synced to `Cargo.toml`).

## 🏗️ Core Architecture (The Bridge Pattern)
- **High-Performance Core**: Cap'n Proto over TCP with 4-byte BE framing and Mandatory Handshake.
- **Interoperability Bridge**: gRPC "Log Bridge" (Port 15001) for JS/HTTP environments.
- **Zero-String Policy**: Delayed formatting for maximum throughput.
- **Single Ordered Writer**: Guaranteed sequential logs via centralized MPSC channel.
