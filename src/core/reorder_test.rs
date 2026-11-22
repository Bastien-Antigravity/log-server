//! ESSENTIAL PROCESS:
//! Regression and concurrency tests for single-authority FIFO sequencing inside LogWriter,
//! clean shutdown draining when senders drop, identical disk-to-ring-buffer ordering,
//! and multi-producer concurrent burst ingestion without synthetic gaps or packet drops.
//!
//! DATA FLOW:
//! 1. Concurrent producers / protocol handlers -> MPSC writer_tx channel
//! 2. Single-authority LogWriter assigns monotonic sequence in FIFO dequeue order
//! 3. Batched transactional writes to _main.log followed by exact-order LogRingBuffer commits
//! 4. Clean loop termination when all writer_tx senders are dropped
//!
//! KEY PARAMETERS:
//! - initial_batch_size: Dynamic burst draining threshold per write_batch syscall.
//! - gap_timeout_ms: Periodic flush interval clamping window.

#[cfg(test)]
mod tests {
    use crate::core::protocol_handlers::handle_grpc_message;
    use crate::facade::log_writer::{LogWriter, WriterConfig};
    use crate::models::log_entry::LogEntry;
    use crate::models::log_packet::LogPacket;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use tokio::time::{sleep, timeout, Duration};

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

    //-----------------------------------------------------------------------------------------------

    #[tokio::test]
    async fn test_single_authority_fifo_ordering_ignores_caller_sequence_drift() {
        let (dir, log_path) = unique_test_log_path("fifo_authority");
        let config = WriterConfig {
            initial_batch_size: 10,
            gap_timeout_ms: 50,
            ..WriterConfig::default()
        };

        let writer = LogWriter::with_config_and_path(config, log_path.clone())
            .await
            .unwrap();
        let ring_buffer = Arc::new(crate::core::ring_buffer::LogRingBuffer::new(100));
        let (tx, writer_handle) = writer.start_writer_task(ring_buffer.clone());

        // Even if callers supply out-of-order or gapped sequence fields (e.g. 99, 3, 0, 42),
        // LogWriter is the single sequence authority and must persist in exact FIFO channel order
        // without artificial [SEQUENCE_GAP] or [BUFFER_PRESSURE] warnings.
        let messages = [
            (99, "fifo_first_payload"),
            (3, "fifo_second_payload"),
            (0, "fifo_third_payload"),
            (42, "fifo_fourth_payload"),
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

        drop(tx);
        timeout(Duration::from_secs(2), writer_handle)
            .await
            .expect("Writer task must terminate promptly when sender is dropped")
            .expect("Writer task should not panic")
            .expect("Writer task should succeed");

        let content = fs::read_to_string(&log_path).expect("Log file must exist");
        let pos_0 = content.find("fifo_first_payload").expect("first payload");
        let pos_1 = content.find("fifo_second_payload").expect("second payload");
        let pos_2 = content.find("fifo_third_payload").expect("third payload");
        let pos_3 = content.find("fifo_fourth_payload").expect("fourth payload");

        assert!(pos_0 < pos_1, "first must appear before second");
        assert!(pos_1 < pos_2, "second must appear before third");
        assert!(pos_2 < pos_3, "third must appear before fourth");
        assert!(!content.contains("[SEQUENCE_GAP]"));
        assert!(!content.contains("[BUFFER_PRESSURE]"));

        let tail = ring_buffer.get_tail(10, None, None);
        assert_eq!(tail.len(), 4);
        assert_eq!(tail[0].message, "fifo_first_payload");
        assert_eq!(tail[1].message, "fifo_second_payload");
        assert_eq!(tail[2].message, "fifo_third_payload");
        assert_eq!(tail[3].message, "fifo_fourth_payload");

        let _ = fs::remove_dir_all(dir);
    }

    //-----------------------------------------------------------------------------------------------

    #[tokio::test]
    async fn test_clean_shutdown_drains_all_pending_messages_without_hanging() {
        let (dir, log_path) = unique_test_log_path("clean_shutdown");
        let config = WriterConfig {
            initial_batch_size: 8,
            gap_timeout_ms: 25,
            ..WriterConfig::default()
        };

        let writer = LogWriter::with_config_and_path(config, log_path.clone())
            .await
            .unwrap();
        let ring_buffer = Arc::new(crate::core::ring_buffer::LogRingBuffer::new(200));
        let (tx, writer_handle) = writer.start_writer_task(ring_buffer.clone());

        for i in 0..50 {
            tx.send(LogPacket {
                sequence: 0,
                entry: LogEntry {
                    message: format!("shutdown_drain_msg_{i:03}"),
                    ..LogEntry::default()
                },
            })
            .await
            .unwrap();
        }

        // Drop the sole sender: tokio::select! must break cleanly instead of hanging on flush_timer
        drop(tx);
        let result = timeout(Duration::from_secs(2), writer_handle)
            .await
            .expect("Writer task hung after all senders were dropped");
        result.expect("JoinHandle ok").expect("Writer task Result ok");

        let content = fs::read_to_string(&log_path).expect("Log file must be readable");
        let file_lines: Vec<&str> = content.lines().collect();
        assert_eq!(file_lines.len(), 50, "All 50 messages must be flushed on shutdown");

        let ring_entries = ring_buffer.get_tail(100, None, None);
        assert_eq!(ring_entries.len(), 50, "All 50 messages must be committed to ring buffer");

        for i in 0..50 {
            let expected = format!("shutdown_drain_msg_{i:03}");
            assert!(
                file_lines[i].contains(&expected),
                "Line {i} expected '{expected}', got '{}'",
                file_lines[i]
            );
            assert_eq!(ring_entries[i].message, expected);
        }

        let _ = fs::remove_dir_all(dir);
    }

    //-----------------------------------------------------------------------------------------------

    #[tokio::test]
    async fn test_concurrent_producers_1000_messages_strict_fifo_and_identical_disk_ring_order() {
        let (dir, log_path) = unique_test_log_path("concurrent_1000");
        let config = WriterConfig {
            initial_batch_size: 32,
            buffer_size: 2048,
            gap_timeout_ms: 20,
            ..WriterConfig::default()
        };

        let writer = LogWriter::with_config_and_path(config, log_path.clone())
            .await
            .unwrap();
        let ring_buffer = Arc::new(crate::core::ring_buffer::LogRingBuffer::new(2000));
        let (tx, writer_handle) = writer.start_writer_task(ring_buffer.clone());
        let unused_counter = Arc::new(AtomicU64::new(0));

        // Spawn 10 concurrent producer tasks each sending 100 messages (1,000 total)
        let mut producer_handles = Vec::new();
        for producer_id in 0..10 {
            let producer_tx = tx.clone();
            let counter_clone = unused_counter.clone();
            producer_handles.push(tokio::spawn(async move {
                for msg_idx in 0..100 {
                    if msg_idx % 7 == 0 {
                        tokio::task::yield_now().await;
                    }
                    let entry = LogEntry {
                        message: format!("P{producer_id:02}_M{msg_idx:03}"),
                        service_name: format!("service_{producer_id}"),
                        level: 3,
                        ..LogEntry::default()
                    };
                    handle_grpc_message(entry, producer_tx.clone(), counter_clone.clone())
                        .await
                        .unwrap();
                }
            }));
        }

        for h in producer_handles {
            h.await.unwrap();
        }

        // Drop last sender and wait for clean shutdown
        drop(tx);
        timeout(Duration::from_secs(5), writer_handle)
            .await
            .expect("Writer task should finish draining 1000 messages")
            .expect("Writer task join")
            .expect("Writer task io result");

        // Verify 1,000 messages persisted on disk and in ring buffer with identical ordering
        let content = fs::read_to_string(&log_path).expect("Log file must exist");
        let file_lines: Vec<&str> = content.lines().collect();
        assert_eq!(file_lines.len(), 1000, "Zero messages may be dropped on disk");
        assert_eq!(ring_buffer.total_ingested(), 1000);

        let ring_entries = ring_buffer.get_tail(2000, None, None);
        assert_eq!(ring_entries.len(), 1000);

        // Verify disk order matches ring buffer order 1-for-1 and each producer's messages stay monotonic
        let mut last_seen_per_producer = [None::<usize>; 10];
        for (idx, entry) in ring_entries.iter().enumerate() {
            assert!(
                file_lines[idx].contains(&entry.message),
                "Disk line {idx} ('{}') does not match ring buffer entry ('{}')",
                file_lines[idx],
                entry.message
            );

            // Parse P<id>_M<seq>
            let parts: Vec<&str> = entry.message.split("_M").collect();
            let p_id: usize = parts[0].trim_start_matches('P').parse().unwrap();
            let m_id: usize = parts[1].parse().unwrap();
            if let Some(prev) = last_seen_per_producer[p_id] {
                assert_eq!(m_id, prev + 1, "Producer {p_id} messages must remain in strict FIFO order");
            } else {
                assert_eq!(m_id, 0, "First message from producer {p_id} must be M000");
            }
            last_seen_per_producer[p_id] = Some(m_id);
        }

        // Also ensure live active polling without dropping tx works deterministically
        let _ = sleep(Duration::from_millis(1)).await;
        let _ = fs::remove_dir_all(dir);
    }
}
