#[cfg(test)]
mod tests {
    use crate::core::log_writer::{LogWriter, WriterConfig};
    use std::fs;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_gap_timeout_recovery() {
        // 1. Setup writer with a short timeout for the test
        let mut config = WriterConfig::default();
        config.gap_timeout_ms = 100; // 100ms for fast testing
        
        let writer = LogWriter::with_config(config).await.unwrap();
        // Override config for the test (we need to expose config or use a custom constructor)
        // For now, we'll assume the default 500ms if we can't inject, but let's try to make it testable.
        
        let tx = writer.start_writer_task();

        // 2. Send sequence 1 (Skipping 0)
        // Format: "sequence_id message"
        tx.send("1 test_message_after_gap".to_string()).await.unwrap();

        // 3. Wait for the timeout to trigger (100ms + some margin)
        sleep(Duration::from_millis(600)).await;

        // 4. Verify log file contains the message despite the gap
        let log_path = writer.base_file_path.clone();
        let content = fs::read_to_string(&log_path).expect("Log file should exist");
        
        assert!(content.contains("test_message_after_gap"), "Log should contain message after gap timeout");
        
        // Cleanup
        let _ = fs::remove_file(log_path);
    }
}
