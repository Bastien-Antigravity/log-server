//! ESSENTIAL PROCESS:
//! Regression tests for sequence reordering, gap detection, and timeout recovery.
//!
//! DATA FLOW:
//! 1. Out-of-order LogPackets -> Writer task BTreeMap
//! 2. Gap timeout force-flush triggers on missing sequence
//!
//! KEY PARAMETERS:
//! - gap_timeout_ms: Shortened interval (100ms) for deterministic test execution.

#[cfg(test)]
mod tests {
    use crate::facade::log_writer::{LogWriter, WriterConfig};
    use crate::models::log_entry::LogEntry;
    use crate::models::log_packet::LogPacket;
    use std::fs;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_gap_timeout_recovery() {
        // 1. Setup writer with a short timeout for the test
        let config = WriterConfig {
            gap_timeout_ms: 100,
            ..WriterConfig::default()
        };

        let writer = LogWriter::with_config(config).await.unwrap();
        // Override config for the test (we need to expose config or use a custom constructor)
        // For now, we'll assume the default 500ms if we can't inject, but let's try to make it testable.

        let ring_buffer = std::sync::Arc::new(crate::core::ring_buffer::LogRingBuffer::new(100));
        let tx = writer.start_writer_task(ring_buffer);

        // 2. Send sequence 1 (Skipping 0)
        let entry = LogEntry {
            message: "test_message_after_gap".to_string(),
            ..LogEntry::default()
        };
        tx.send(LogPacket { sequence: 1, entry }).await.unwrap();

        // 3. Wait for the timeout to trigger (100ms + some margin)
        sleep(Duration::from_millis(600)).await;

        // 4. Verify log file contains the message despite the gap
        let log_path = writer.base_file_path.clone();
        let content = fs::read_to_string(&log_path).expect("Log file should exist");

        assert!(
            content.contains("test_message_after_gap"),
            "Log should contain message after gap timeout"
        );

        // Cleanup
        let _ = fs::remove_file(log_path);
    }
}
