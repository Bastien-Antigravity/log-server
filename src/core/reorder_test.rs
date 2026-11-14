//! ESSENTIAL PROCESS:
//! Regression tests for sequence reordering, gap detection, timeout recovery,
//! buffer pressure handling, and late packet recovery.
//!
//! DATA FLOW:
//! 1. Out-of-order LogPackets -> Writer task BTreeMap reorder buffer
//! 2. Gap timeout force-flush triggers on missing sequence
//! 3. Buffer saturation forces progression when buffer.len() >= batch_size
//! 4. Late packets (sequence < current_sequence) write immediately
//!
//! KEY PARAMETERS:
//! - gap_timeout_ms: Shortened interval (100ms) for fast, deterministic test execution.
//! - initial_batch_size: Configured threshold for buffer saturation testing.

#[cfg(test)]
mod tests {
    use crate::facade::log_writer::{LogWriter, WriterConfig};
    use crate::models::log_entry::LogEntry;
    use crate::models::log_packet::LogPacket;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use tokio::time::{sleep, Duration};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_test_log_path(test_name: &str) -> (PathBuf, PathBuf) {
        let count = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "log_server_test_{}_{}_{}",
            test_name,
            std::process::id(),
            count
        ));
        let _ = fs::create_dir_all(&dir);
        let log_file = dir.join("_main.log");
        (dir, log_file)
    }

    // -----------------------------------------------------------------------------

    #[tokio::test]
    async fn test_gap_timeout_recovery() {
        let (dir, log_path) = unique_test_log_path("gap_timeout");
        let config = WriterConfig {
            gap_timeout_ms: 100,
            ..WriterConfig::default()
        };

        let writer = LogWriter::with_config_and_path(config, log_path.clone())
            .await
            .unwrap();
        let ring_buffer = std::sync::Arc::new(crate::core::ring_buffer::LogRingBuffer::new(100));
        let tx = writer.start_writer_task(ring_buffer.clone());

        // Send sequence 1 (Skipping 0)
        let entry = LogEntry {
            message: "test_message_after_gap".to_string(),
            ..LogEntry::default()
        };
        tx.send(LogPacket { sequence: 1, entry }).await.unwrap();

        // Active poll for gap timeout to trigger and write to file (up to 2000ms)
        let mut content = String::new();
        let mut found = false;
        for _ in 0..80 {
            if log_path.exists() {
                if let Ok(c) = fs::read_to_string(&log_path) {
                    content = c;
                    if content.contains("test_message_after_gap") && content.contains("[SEQUENCE_GAP]") {
                        found = true;
                        break;
                    }
                }
            }
            sleep(Duration::from_millis(25)).await;
        }

        assert!(
            found,
            "Log should contain both message and [SEQUENCE_GAP] after timeout. Read:\n{content}"
        );

        // Verify ring buffer contains the structured WARNING gap entry
        let tail = ring_buffer.get_tail(10, None, None);
        assert!(
            tail.iter().any(|e| e.message.contains("[SEQUENCE_GAP]") && e.level == 9),
            "Ring buffer should contain structured [SEQUENCE_GAP] WARNING entry"
        );

        let _ = fs::remove_dir_all(dir);
    }

    // -----------------------------------------------------------------------------

    #[tokio::test]
    async fn test_reorder_out_of_order_stream() {
        let (dir, log_path) = unique_test_log_path("reorder_stream");
        let config = WriterConfig {
            initial_batch_size: 10,
            gap_timeout_ms: 2000, // Long gap timer so reordering occurs naturally
            ..WriterConfig::default()
        };

        let writer = LogWriter::with_config_and_path(config, log_path.clone())
            .await
            .unwrap();
        let ring_buffer = std::sync::Arc::new(crate::core::ring_buffer::LogRingBuffer::new(100));
        let tx = writer.start_writer_task(ring_buffer.clone());

        // Send out-of-order sequence: 3, 0, 2, 1
        let messages = [
            (3, "seq_3_payload"),
            (0, "seq_0_payload"),
            (2, "seq_2_payload"),
            (1, "seq_1_payload"),
        ];

        for (seq, msg) in messages {
            tx.send(LogPacket {
                sequence: seq,
                entry: LogEntry {
                    message: msg.to_string(),
                    ..LogEntry::default()
                },
            })
            .await
            .unwrap();
        }

        // Active poll until all 4 payloads appear in the file
        let mut content = String::new();
        let mut found = false;
        for _ in 0..80 {
            if log_path.exists() {
                if let Ok(c) = fs::read_to_string(&log_path) {
                    content = c;
                    if content.contains("seq_0_payload")
                        && content.contains("seq_1_payload")
                        && content.contains("seq_2_payload")
                        && content.contains("seq_3_payload")
                    {
                        found = true;
                        break;
                    }
                }
            }
            sleep(Duration::from_millis(25)).await;
        }

        assert!(found, "Log file should contain all 4 payloads. Read:\n{content}");

        // Verify chronological sequence in file: 0 appears before 1, 1 before 2, 2 before 3
        let pos_0 = content.find("seq_0_payload").unwrap();
        let pos_1 = content.find("seq_1_payload").unwrap();
        let pos_2 = content.find("seq_2_payload").unwrap();
        let pos_3 = content.find("seq_3_payload").unwrap();

        assert!(pos_0 < pos_1, "seq 0 must appear before seq 1");
        assert!(pos_1 < pos_2, "seq 1 must appear before seq 2");
        assert!(pos_2 < pos_3, "seq 2 must appear before seq 3");

        // Verify zero gaps occurred
        assert!(!content.contains("[SEQUENCE_GAP]"));
        assert!(!content.contains("[BUFFER_PRESSURE]"));

        let _ = fs::remove_dir_all(dir);
    }

    // -----------------------------------------------------------------------------

    #[tokio::test]
    async fn test_buffer_pressure_force_progress() {
        let (dir, log_path) = unique_test_log_path("buffer_pressure");
        let config = WriterConfig {
            initial_batch_size: 4,
            gap_timeout_ms: 30000, // Very long gap timer so buffer saturation forces progress
            ..WriterConfig::default()
        };

        let writer = LogWriter::with_config_and_path(config, log_path.clone())
            .await
            .unwrap();
        let ring_buffer = std::sync::Arc::new(crate::core::ring_buffer::LogRingBuffer::new(100));
        let tx = writer.start_writer_task(ring_buffer.clone());

        // Skip sequence 0, send 4 items (1, 2, 3, 4) to hit batch_size (4)
        for seq in 1..=4 {
            tx.send(LogPacket {
                sequence: seq,
                entry: LogEntry {
                    message: format!("pressure_msg_{seq}"),
                    ..LogEntry::default()
                },
            })
            .await
            .unwrap();
        }

        // Active poll until [BUFFER_PRESSURE] appears in the file
        let mut content = String::new();
        let mut found = false;
        for _ in 0..80 {
            if log_path.exists() {
                if let Ok(c) = fs::read_to_string(&log_path) {
                    content = c;
                    if content.contains("[BUFFER_PRESSURE]") && content.contains("pressure_msg_4") {
                        found = true;
                        break;
                    }
                }
            }
            sleep(Duration::from_millis(25)).await;
        }

        assert!(
            found,
            "Log should contain [BUFFER_PRESSURE] when buffer capacity forces progress. Read:\n{content}"
        );
        assert!(content.contains("pressure_msg_1"));
        assert!(content.contains("pressure_msg_4"));

        // Verify ring buffer contains the structured ERROR buffer pressure entry
        let tail = ring_buffer.get_tail(10, None, None);
        assert!(
            tail.iter().any(|e| e.message.contains("[BUFFER_PRESSURE]") && e.level == 10),
            "Ring buffer should contain structured [BUFFER_PRESSURE] ERROR entry"
        );

        let _ = fs::remove_dir_all(dir);
    }

    // -----------------------------------------------------------------------------

    #[tokio::test]
    async fn test_late_arriving_sequence_written_directly() {
        let (dir, log_path) = unique_test_log_path("late_sequence");
        let config = WriterConfig {
            gap_timeout_ms: 100,
            ..WriterConfig::default()
        };

        let writer = LogWriter::with_config_and_path(config, log_path.clone())
            .await
            .unwrap();
        let ring_buffer = std::sync::Arc::new(crate::core::ring_buffer::LogRingBuffer::new(100));
        let tx = writer.start_writer_task(ring_buffer.clone());

        // Step 1: Send seq 1 (skipping 0) and wait for gap timeout to advance sequence to 2
        tx.send(LogPacket {
            sequence: 1,
            entry: LogEntry {
                message: "seq_1_after_gap".to_string(),
                ..LogEntry::default()
            },
        })
        .await
        .unwrap();

        let mut advanced = false;
        for _ in 0..80 {
            if log_path.exists() {
                if let Ok(c) = fs::read_to_string(&log_path) {
                    if c.contains("seq_1_after_gap") {
                        advanced = true;
                        break;
                    }
                }
            }
            sleep(Duration::from_millis(25)).await;
        }
        assert!(advanced, "Initial sequence 1 should be committed after gap timeout");

        // Step 2: Now send late-arriving sequence 0 (sequence 0 < current_sequence 2)
        tx.send(LogPacket {
            sequence: 0,
            entry: LogEntry {
                message: "late_seq_0_recovered".to_string(),
                ..LogEntry::default()
            },
        })
        .await
        .unwrap();

        // Step 3: Active poll to verify late-arriving sequence 0 is written to file and ring buffer
        let mut recovered = false;
        let mut final_content = String::new();
        for _ in 0..80 {
            if let Ok(c) = fs::read_to_string(&log_path) {
                final_content = c;
                if final_content.contains("late_seq_0_recovered") {
                    recovered = true;
                    break;
                }
            }
            sleep(Duration::from_millis(25)).await;
        }

        assert!(
            recovered,
            "Late arriving packet should be written to file without being dropped. Read:\n{final_content}"
        );

        let tail = ring_buffer.get_tail(10, None, None);
        assert!(
            tail.iter().any(|e| e.message.contains("late_seq_0_recovered")),
            "Ring buffer should receive late packet"
        );

        let _ = fs::remove_dir_all(dir);
    }
}
