# Log Server TODO

## 🛡️ Hardening (Spec-First)
- [x] **Ingestion Gap Protection**: Implement a 500ms timeout for missing sequence IDs in `LogWriter` (FEAT-002).
- [x] **Protocol Hardening**: Enforce 4-byte BE framing and Mandatory Handshake (FEAT-006).
- [x] **Semantic Refactor**: Rename gRPC ingestion to "Log Bridge" to distinguish from Control Ports.
- [ ] **Rotation Safety**: Verify rotation doesn't drop messages during file switch (FEAT-004).

## 🏗️ Technical Debt
- [ ] Refactor GRPC/TCP error messages for clarity.
- [ ] Audit microservice-toolbox CLI parameters for parity.