//! ESSENTIAL PROCESS:
//! Log Server library root crate. Defines module boundaries, public facades,
//! and core exports for centralized Cap'n Proto TCP and tonic gRPC services.
//!
//! DATA FLOW:
//! 1. Ingestors (TCP / gRPC) -> Sequence Counter
//! 2. Protocol Handlers -> LogPacket -> BTreeMap Reorder Buffer
//! 3. WriterTask -> LogRingBuffer & Disk Persistence (_main.log)
//!
//! KEY PARAMETERS:
//! - config, core, facade, models, protocols, servers, transport, utils.

pub mod config;
pub mod core;
pub mod facade;
pub mod models;
pub mod protocols;
pub mod servers;
pub mod transport;
pub mod utils;

// Re-export main components
pub use config::config::Config;
pub use facade::log_server::LogServer;
pub use facade::log_writer::LogWriter;
