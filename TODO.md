# Log Server TODO

## 🛡️ Hardening (Spec-First)
- [x] **Ingestion Gap Protection**: Implement a 500ms timeout for missing sequence IDs in `LogWriter` (FEAT-002).
- [ ] **Rotation Safety**: Verify rotation doesn't drop messages during file switch (FEAT-004).

## 🏗️ Technical Debt
- [ ] Refactor GRPC/TCP error messages for clarity.
- [ ] Audit microservice-toolbox CLI parameters for parity.