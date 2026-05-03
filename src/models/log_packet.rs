//! Internal sequenced log packet
use crate::models::log_entry::LogEntry;

/// A packet containing a sequence number and the log entry
pub struct LogPacket {
    pub sequence: u64,
    pub entry: LogEntry,
}
