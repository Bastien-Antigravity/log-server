# Log Server Topology

## System Architecture

The Log Server uses a **Zero-String Ingestion Pipeline** to ensure high throughput and semantic integrity.

```mermaid
graph TD
    subgraph Clients
        TCP[TCP Client]
        GRPC[gRPC Client]
        INT[Internal Logger]
    end

    subgraph Handlers
        TH[TCP Handler]
        GH[gRPC Handler]
    end

    subgraph Facade
        LS[LogServer Orchestrator]
        LW[LogWriter Task]
    end

    TCP -->|SafeSocket| TH
    TH -->|Handshake| TH
    TH -->|LogPacket| LW
    
    GRPC --> GH
    GH -->|LogPacket| LW
    
    INT -->|LogPacket| LW

    LW -->|BTreeMap Reordering| BUF[Sequence Buffer]
    BUF -->|500ms Gap Timeout| OUT[Log Formatter]
    OUT -->|Colored| Console
    OUT -->|Raw| File[_main.log]
```

## Protocol Specifications
- **Transport**: SafeSocket (TCP) with Length-Prefixed Framing.
- **Serialization**: Cap'n Proto (TCP) / Protobuf (gRPC).
- **Identity**: Mandatory Handshake (TCP) required before data transmission.
- **Sequence Enforcement**: 500ms timeout for missing sequences to maintain service availability.
