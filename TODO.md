# Log Server TODO

## 🛡️ Hardening (Spec-First)
- [x] **Ingestion Gap Protection**: Implement a 500ms timeout for missing sequence IDs in `LogWriter` (FEAT-002).
- [ ] **Rotation Safety**: Verify rotation doesn't drop messages during file switch (FEAT-004).
- [ ] **Purger Cleanup**: Perform a **[[Daily-AI-Playbook#3-Purger-Gate-The-Straight-to-Goal-Check|Purger-Gate Audit]]** to identify and remove legacy gRPC logic superseded by Cap'n Proto.

## 🏗️ Technical Debt
- [ ] Refactor GRPC/TCP error messages for clarity.
- [ ] Audit microservice-toolbox CLI parameters for parity.