//! ESSENTIAL PROCESS:
//! Internal sequenced log packet wrapping a LogEntry with a global monotonic sequence.
//!
//! DATA FLOW:
//! 1. Protocol Handlers wrap LogEntry with atomic sequence -> LogPacket
//! 2. LogPacket -> MPSC channel -> WriterTask BTreeMap buffer
//!
//! KEY PARAMETERS:
//! - sequence: Global atomic counter value for ordered stream reconstruction.
//! - entry: Decoded LogEntry payload.

use crate::models::log_entry::LogEntry;

/// A packet containing a sequence number and the log entry
pub struct LogPacket {
    pub sequence: u64,
    pub entry: LogEntry,
}
