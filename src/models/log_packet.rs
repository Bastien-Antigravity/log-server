//! ESSENTIAL PROCESS:
//! Internal sequenced log packet wrapping a LogEntry with a monotonic sequence number
//! assigned authoritatively by LogWriter upon FIFO channel reception.
//!
//! DATA FLOW:
//! 1. Protocol Handlers wrap LogEntry -> LogPacket -> MPSC writer_tx channel
//! 2. LogWriter assigns monotonic FIFO sequence and persists batch to disk & LogRingBuffer
//!
//! KEY PARAMETERS:
//! - sequence: Monotonic sequence value assigned by single-authority LogWriter.
//! - entry: Decoded LogEntry payload.

use crate::models::log_entry::LogEntry;

/// A packet containing a sequence number and the log entry
pub struct LogPacket {
    pub sequence: u64,
    pub entry: LogEntry,
}
