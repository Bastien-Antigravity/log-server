
Log-server — Updated technical audit
Active development
24 November 2026
Major remediation detected

Repository: Bastien-Antigravity/log-server

Branch audited: develop

Latest commit: 2256cdf — 24 November 2026.

This is a fresh audit against the current repository state, not a repetition of the previous assessment.

The most important development is that the project has addressed several serious architectural weaknesses identified in the previous external audit:

- Sequence assignment is now centralized in the writer.
- FIFO persistence and ring-buffer publication have been aligned.
- A shutdown hang has been addressed.
- Transactional file recovery now propagates additional I/O failures.
- CI and build validation have received further attention.
  These are meaningful improvements, particularly for a logging service where silent message loss and inconsistent ordering are unacceptable.

1. Updated architecture assessment
   Improved
   Single-authority FIFO writer

This is probably the most important improvement.

Previously, sequence numbers were assigned by producers before entering the MPSC channel. With concurrent TCP, gRPC and internal logging producers, this could create ordering races and artificial sequence gaps.

The new implementation in src/facade/log_writer.rs assigns sequence numbers after dequeue, inside the single writer task.

The resulting pipeline is:
TCP / gRPC / Internal producers

Bounded Tokio MPSC channel

Single-authority sequencing and batching

Transactional disk writer

Append, rollback, rotation, flush

Ring buffer

Committed records and streaming

Conceptual architecture based on the current implementation.
Assessment: The architecture is considerably cleaner. It removes unnecessary sequencing complexity and establishes a clear authority for persistence ordering.

One important qualification: FIFO here means channel dequeue order, not a globally deterministic chronological order across concurrent producers. That distinction should be explicit in the protocol contract.
2. Findings from the new code review
Area	Assessment	Status
FIFO sequencing	Single authority	Improved
Concurrent ingestion	Bounded MPSC	Good
Burst batching	Adaptive batching	Good
Disk persistence	Transactional rollback	Improved
File rotation	Cascading rotation	Good
Fatal disk errors	Propagated to lifecycle	Improved
Graceful shutdown	Writer drain addressed	Partial
Live streaming	Tokio broadcast	Needs hardening
Integration tests	Environment-dependent skips	Needs attention
CI/CD	External reusable workflows	Needs verification

3. Remaining weaknesses and risks
   These are findings from inspecting the current source, rather than issues inferred from the previous audit.
   P1 — Server lifecycle coordination
   File: src/facade/log_server.rs

The shutdown path has improved, but there is still a lifecycle problem.

The main tokio::select! handles four events:

- TCP server termination.
- gRPC server termination.
- Writer task termination.
- Operating-system shutdown signal.
  When TCP or gRPC terminates independently, the function returns immediately.

It does not consistently:

- Abort the other server.
- Clear the internal logger.
- Drop all writer senders.
- Await the writer's final drain.
  This means a server failure can leave background tasks running after the main run() function has returned.

Recommendation: Introduce a centralized cancellation mechanism and a single shutdown coordinator. Every termination path should pass through the same cleanup procedure.
P1 — Ring-buffer failure handling
File: src/core/ring_buffer.rs

The current push() implementation increments total_ingested, publishes to broadcast, and then attempts to acquire the internal RwLock.

If acquiring the lock fails because it is poisoned, the error is silently ignored.

Consequences:

- total_ingested can report more records than the tail buffer contains.
- Live subscribers can receive records that are absent from historical queries.
- The system has no explicit recovery or observability for this condition.
  Recommendation: Define a consistent failure policy for ring-buffer poisoning. At minimum, record the failure and expose the discrepancy through metrics.
  P1 — Integration tests can report success without testing
  File: tests/integration_tests.rs

There are several environment-dependent early returns.

For example, the integration test can skip execution when:

- Loopback binding is prohibited.
- Server initialization fails.
- The server cannot be reached.
  This is understandable for restricted development environments, but it creates a reliability problem: a test suite can finish successfully without exercising the actual integration pipeline.

Recommendation:

Separate tests into:

1. Mandatory CI integration tests.
2. Optional sandbox-compatible tests.
3. Explicitly reported skipped tests.
   A mandatory CI environment must fail when its integration prerequisites are unavailable.
   P2 — Transactional persistence is not equivalent to guaranteed durability
   File: src/core/transactional_writer.rs

The implementation is substantially improved:

- In-memory staging.
- Checkpoint offsets.
- Partial-write rollback.
- Handle recovery.
- Optional sync_data().
  However, there is an important distinction.

write_all() followed by flush() does not guarantee physical persistence to storage. The optional sync_data() improves this.

There is also a rotation durability issue: renaming files and creating a new active file do not, by themselves, guarantee that directory metadata survives a sudden power loss.

For a genuinely durable logging service, consider:

- Synchronizing the active file.
- Synchronizing the parent directory after rotation where supported.
- Testing crash recovery across rename and reopen operations.
- Explicitly documenting the durability guarantees of each configuration.
  Also, the current recovery strategy should be tested against real filesystem errors, not just simulated partial writes.
  P2 — Broadcast streaming can lose messages
  File: src/core/ring_buffer.rs

Tokio broadcast channels are bounded.

The current capacity is 4,096 messages. A slow subscriber can fall behind and receive a Lagged error.

This is not necessarily a design flaw: bounded broadcast is appropriate for live observability.

However, the consumer contract must define what happens after lag:

- Does the client reconnect?
- Does it retrieve missing entries using GetTailLogs?
- Can it detect the exact missing sequence range?
- Is there a way to resume from a known offset?
  I would introduce a cursor-based recovery mechanism rather than trying to make the broadcast channel unbounded.
  P2 — CI/CD dependency and trust boundary
  Files: .github/workflows/ci.yml, .github/workflows/docker-publish.yml

The project now delegates CI and Docker publishing to fleet-operation-brain.

This improves centralization and avoids duplicated workflow implementations.

But it creates a new dependency:

The reliability and security of log-server delivery now depend on an external repository and its reusable workflow implementation.

I would specifically verify:

- Whether reusable workflows are pinned to immutable commit SHAs.
- Whether publishing is restricted to trusted branch commits.
- Whether pull-request execution can influence privileged publishing.
- Whether inherited secrets are actually necessary.
- Whether the reusable workflow has minimal GitHub permissions.
  The current @develop references are mutable. This is a supply-chain and reproducibility concern.

P1/P2 — TCP framing does not distinguish clean EOF from truncated headers
File: src/transport/safe_socket.rs

There is a subtle protocol correctness issue in receive_data().

The implementation does this:
let n = self.reader.read_exact(&mut length_buf).await;

if let Err(e) = n {
    if e.kind() == io::ErrorKind::UnexpectedEof {
        return Ok(None);
    }
    return Err(e);
}

This treats every UnexpectedEof as a clean connection closure.

But there are two different situations:

- Connection closes before any header byte arrives: normal EOF.
- Connection closes after receiving only 1–3 header bytes: malformed, truncated frame.
  The second case should be reported as a protocol error.

Recommendation: Read the first header byte separately, distinguish zero-byte EOF from partial-header EOF, and reject incomplete frames.

This matters for detecting network interruptions and malformed clients.
P2 — Protocol contract consistency
File: src/core/protocol_handlers.rs

The TCP handshake still uses an architectural shortcut: it interprets the first two text fields of the SafeSocket HelloMsg using the generated LoggerMsg schema.

The code documents this explicitly.

It works as long as the two schemas remain structurally compatible, but it introduces implicit coupling.

I would prefer a canonical shared handshake schema or a dedicated handshake type.

The recent migration of logger.capnp to microservice-toolbox is a positive step towards eliminating duplicated contracts. However, the handshake contract should be addressed separately.

4. Important architectural observations
   There are three additional points worth discussing.

A. Batching efficiency

The writer drains immediately available messages using try_recv() and then flushes.

This is efficient under bursts, but it does not deliberately wait for a batch to fill.

Under low-volume workloads, each incoming message can trigger its own disk write and flush.

A configurable maximum batching delay could improve throughput without introducing excessive logging latency.

B. Console logging

The background console worker uses a bounded channel and try_send().

When the console worker cannot keep up, console messages are silently discarded.

This does not directly compromise disk persistence, which is good, but the behavior should be observable through dropped-console-message metrics.

C. Sequence counter cleanup

The server still initializes and passes an AtomicU64 sequence counter to protocol handlers, even though the writer now owns sequence assignment.

This is redundant state and should be removed once all consumers have migrated.

It is a relatively minor issue, but eliminating obsolete mechanisms reduces future regression risk.

5. Updated scoring
   My assessment of the current implementation:
   Overall engineering quality
   8.1/10

My qualitative assessment, not a test-derived score.

Category	Score
Architecture	8.7/10
Concurrency	8.6/10
Persistence	8.2/10
Error handling	8.0/10
Networking	7.8/10
Testing strategy	7.3/10
Maintainability	8.2/10
Production readiness	7.6/10

Evolution compared with the previous audit
Positive progression
Area	Evolution
Ordering architecture	Major improvement
Writer shutdown	Major improvement
Disk recovery	Improvement
Error propagation	Improvement
Contract centralization	Improvement
CI/CD	More centralized, new trust dependency
End-to-end verification	Still needs work

I would not claim that every previously identified issue is resolved. The latest source shows genuine improvements, but I have not independently executed the Rust test suite or performed destructive filesystem testing.
6. What I would prioritize next
My recommended remediation order:
Priority 1 — Reliability
Critical

- [ ] Centralized lifecycle and cancellation management.
- [ ] Fix partial TCP header EOF handling.
- [ ] Make ring-buffer failures observable.
- [ ] Ensure mandatory integration tests cannot silently skip.

  Priority 2 — Durability
  High
- [ ] Crash-consistency tests for file rotation.
- [ ] Validate recovery after actual disk failures.
- [ ] Document precise durability guarantees.

  Priority 3 — Performance
  Medium
- [ ] Configurable batching latency.
- [ ] Console drop metrics.
- [ ] Streaming cursor and recovery protocol.

  Priority 4 — Maintainability
  Medium
- [ ] Remove obsolete sequence counters.
- [ ] Formalize shared handshake schema.
- [ ] Pin external CI workflows to immutable revisions.

7. Final verdict
   I consider log-server a significantly more mature project than in the previous audit.

The most encouraging aspect is not simply that individual bugs have been fixed. It is that the developer has changed the underlying architecture to eliminate entire classes of problems.

Moving sequencing into one authoritative writer is a particularly good engineering decision.

However, I would distinguish three levels of maturity:
Level	Assessment
Architectural design	Strong
Implementation maturity	Good
Independently demonstrated production reliability	Not yet established

The biggest remaining gap is now less about the core logging architecture and more about lifecycle correctness, failure recovery, and proving behavior under adverse conditions.

My recommendation: Keep the current architecture. Do not undertake another major refactoring. Concentrate on deterministic failure testing and operational reliability.

Useful source references:

- LogWriter
- TransactionalFileWriter
- LogServer lifecycle
- SafeSocket
- Integration tests
  One particularly useful next step would be a cross-repository reliability audit of log-server, safe-socket, microservice-toolbox and flexible-logger, focusing on whether their shared transport, schemas, error handling and lifecycle contracts are genuinely consistent
