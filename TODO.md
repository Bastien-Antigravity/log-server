---
microservice: 08-Base-Scripts
type: note
status: active
tags:
- '#service/08-Base-Scripts'
- '#type/note'
- '#state/active'
- '#zone/3-fleet'
---# Log Server TODO

## 🛡️ Hardening (Spec-First)
- [x] **Ingestion Gap Protection**: Implement a 500ms timeout for missing sequence IDs in `LogWriter` (FEAT-002).
- [x] **Protocol Hardening**: Enforce 4-byte BE framing and Mandatory Handshake (FEAT-006).
- [x] **Semantic Refactor**: Rename gRPC ingestion to "Log Bridge" to distinguish from Control Ports.
- [x] **Rotation Safety**: Verify rotation doesn't drop messages during file switch (FEAT-004) - *Verified: single-threaded async writer task with backpressure is safe.*

## 🏗️ Technical Debt
- [x] Refactor GRPC/TCP error messages for clarity - *Verified: structured logging provides full context and status.*
- [x] Audit microservice-toolbox CLI parameters for parity - *Verified: 100% matched parity.*