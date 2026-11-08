//! ESSENTIAL PROCESS:
//! Data models module namespace defining LogEntry and LogPacket primitives.
//!
//! DATA FLOW:
//! 1. Exports internal data structures to the log-server crate.
//!
//! KEY PARAMETERS:
//! - log_entry: Core log message payload.
//! - log_packet: Sequenced carrier for ordered disk writes.

pub mod log_entry;
pub mod log_packet;
