//! ESSENTIAL PROCESS:
//! Asynchronous single-authority FIFO file writer and rotation coordinator. Assigns
//! monotonic sequence numbers upon channel dequeue, drains bursts into contiguous batches,
//! flushes transactionally to _main.log, handles size-based rotation, and feeds the
//! in-memory circular ring buffer in exact persistence order.
//!
//! DATA FLOW:
//! 1. LogPacket stream from MPSC channel (FIFO arrival order)
//! 2. Single-authority sequence assignment and burst draining up to batch_size
//! 3. Transactional disk batch write, rotation check, and flush
//! 4. Committed LogRingBuffer publication and non-blocking background console dispatch
//!
//! KEY PARAMETERS:
//! - max_file_bytes: 10MB per log chunk before rotation.
//! - backup_count: Number of historical rotated log files preserved (default 10).
//! - gap_timeout_ms: Periodic flush interval clamping window in milliseconds.

use crate::core::log_formatter::format_log_message;
use crate::core::transactional_writer::TransactionalFileWriter;
use crate::models::log_entry::LogEntry;
use crate::models::log_packet::LogPacket;
use crate::utils::helpers::{get_hostname, get_utc_timestamp};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::{
    sync::mpsc,
    time::Duration,
};

/// Log writer configuration
#[derive(Clone)]
pub struct WriterConfig {
    pub initial_batch_size: usize,
    pub buffer_size: usize,
    pub max_retries: usize,
    pub retry_delay_ms: u64,
    pub max_file_bytes: u64,
    pub backup_count: usize,
    pub gap_timeout_ms: u64,
    pub sync_on_flush: Arc<AtomicBool>,
}

//-----------------------------------------------------------------------------------------------

impl Default for WriterConfig {
    fn default() -> Self {
        Self {
            initial_batch_size: 100,
            buffer_size: 2048,
            max_retries: 3,
            retry_delay_ms: 100,
            max_file_bytes: 10 * 1024 * 1024, // 10 MB default
            backup_count: 10,
            gap_timeout_ms: 500,
            sync_on_flush: Arc::new(AtomicBool::new(false)),
        }
    }
}

//-----------------------------------------------------------------------------------------------

/// File writer with single-authority FIFO sequencing and rotation
pub struct LogWriter {
    config: WriterConfig,
    pub(crate) base_file_path: PathBuf,
}

//-----------------------------------------------------------------------------------------------

impl LogWriter {
    /// Create new log writer with default config
    pub async fn new() -> Result<Self, std::io::Error> {
        Self::with_config(WriterConfig::default()).await
    }

    //-----------------------------------------------------------------------------------------------

    /// Create new log writer with custom config
    pub async fn with_config(config: WriterConfig) -> Result<Self, std::io::Error> {
        let log_dir = crate::utils::helpers::resolve_log_dir();

        // Ensure log directory exists
        if let Some(log_dir_str) = log_dir.to_str() {
            crate::utils::helpers::create_log_folder(log_dir_str)?;
        }

        let base_file_path = log_dir.join("_main.log");

        Ok(Self {
            config,
            base_file_path,
        })
    }

    //-----------------------------------------------------------------------------------------------

    /// Create new log writer with custom config and explicit file path
    pub async fn with_config_and_path(
        config: WriterConfig,
        base_file_path: PathBuf,
    ) -> Result<Self, std::io::Error> {
        if let Some(parent) = base_file_path.parent() {
            if let Some(parent_str) = parent.to_str() {
                crate::utils::helpers::create_log_folder(parent_str)?;
            }
        }

        Ok(Self {
            config,
            base_file_path,
        })
    }

    //-----------------------------------------------------------------------------------------------

    /// Start the writer task with in-memory ring buffer support, returning the sender channel
    /// and the background task JoinHandle for lifecycle supervision and fail-fast termination.
    pub fn start_writer_task(
        &self,
        ring_buffer: std::sync::Arc<crate::core::ring_buffer::LogRingBuffer>,
    ) -> (mpsc::Sender<LogPacket>, tokio::task::JoinHandle<tokio::io::Result<()>>) {
        let (writer_tx, writer_rx) = mpsc::channel::<LogPacket>(self.config.buffer_size);
        let base_path = self.base_file_path.clone();
        let config = self.config.clone();

        let handle = tokio::spawn(async move {
            Self::writer_task(writer_rx, base_path, config, ring_buffer).await
        });

        (writer_tx, handle)
    }

    //-----------------------------------------------------------------------------------------------

    /// Dynamically toggle hardware durability barrier (fsync) on the fly.
    pub fn set_sync_on_flush(&self, enabled: bool) {
        self.config
            .sync_on_flush
            .store(enabled, std::sync::atomic::Ordering::Relaxed);
    }

    //-----------------------------------------------------------------------------------------------

    /// Query active hardware durability barrier (fsync) state.
    pub fn is_sync_on_flush(&self) -> bool {
        self.config
            .sync_on_flush
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    //-----------------------------------------------------------------------------------------------

    /// Access the underlying shared atomic boolean handle.
    pub fn sync_on_flush_handle(&self) -> Arc<AtomicBool> {
        self.config.sync_on_flush.clone()
    }

    //-----------------------------------------------------------------------------------------------

    /// Rotate log files (delegates to TransactionalFileWriter)
    pub async fn rotate_files(base_path: &PathBuf, backup_count: usize) -> tokio::io::Result<()> {
        TransactionalFileWriter::rotate_files(base_path, backup_count).await
    }

    //-----------------------------------------------------------------------------------------------

    /// Main writer task implementation (single serialization and sequence authority)
    async fn writer_task(
        mut rx: mpsc::Receiver<LogPacket>,
        base_file_path: PathBuf,
        config: WriterConfig,
        ring_buffer: std::sync::Arc<crate::core::ring_buffer::LogRingBuffer>,
    ) -> tokio::io::Result<()> {
        let mut sink = match TransactionalFileWriter::with_sync(
            base_file_path,
            config.max_retries,
            config.retry_delay_ms,
            config.sync_on_flush.clone(),
        )
        .await
        {
            Ok(s) => s,
            Err(e) => {
                notify_disk_failure(&ring_buffer, "Failed to initialize transactional writer", &e);
                return Err(e);
            }
        };

        let mut current_sequence: u64 = 0;
        let mut batch_size = config.initial_batch_size.max(1);
        let mut pending_batch: Vec<String> = Vec::with_capacity(batch_size);
        let mut pending_entries: Vec<LogEntry> = Vec::with_capacity(batch_size);

        let flush_duration = Duration::from_millis(config.gap_timeout_ms.clamp(5, 100));
        let mut flush_timer =
            tokio::time::interval_at(tokio::time::Instant::now() + flush_duration, flush_duration);

        // ASYNC CONSOLE WORKER: Prevent blocking the writer task with println!
        let (console_tx, mut console_rx) = mpsc::channel::<String>(2048);
        tokio::spawn(async move {
            while let Some(msg) = console_rx.recv().await {
                println!("{msg}");
            }
        });

        loop {
            tokio::select! {
                biased;

                // Branch 1: Incoming structured packets in strict MPSC FIFO order
                maybe_packet = rx.recv() => {
                    match maybe_packet {
                        Some(first_packet) => {
                            Self::_stage_packet(
                                first_packet,
                                &mut current_sequence,
                                &console_tx,
                                &mut pending_batch,
                                &mut pending_entries,
                            );

                            // Drain immediately available burst packets up to batch_size
                            while pending_batch.len() < batch_size {
                                match rx.try_recv() {
                                    Ok(next_packet) => {
                                        Self::_stage_packet(
                                            next_packet,
                                            &mut current_sequence,
                                            &console_tx,
                                            &mut pending_batch,
                                            &mut pending_entries,
                                        );
                                    }
                                    Err(_) => break,
                                }
                            }

                            let drained_count = pending_batch.len();
                            Self::_flush_pending_batch(
                                &mut sink,
                                &mut pending_batch,
                                &mut pending_entries,
                                &config,
                                &ring_buffer,
                            )
                            .await?;

                            // Dynamically scale batch size based on burst saturation
                            if drained_count >= batch_size {
                                batch_size = (batch_size * 2).min(1000);
                            } else if drained_count < batch_size / 2 {
                                let min_batch = config.initial_batch_size.clamp(1, 10);
                                batch_size = (batch_size / 2).max(min_batch);
                            }
                        }
                        None => break,
                    }
                }

                // Branch 2: Periodic flush timer
                _ = flush_timer.tick() => {
                    if !pending_batch.is_empty() {
                        Self::_flush_pending_batch(
                            &mut sink,
                            &mut pending_batch,
                            &mut pending_entries,
                            &config,
                            &ring_buffer,
                        )
                        .await?;
                    }
                }
            }
        }

        // Flush any remaining messages on clean shutdown
        if !pending_batch.is_empty() {
            Self::_flush_pending_batch(
                &mut sink,
                &mut pending_batch,
                &mut pending_entries,
                &config,
                &ring_buffer,
            )
            .await?;
        }

        if let Err(e) = sink.flush().await {
            notify_disk_failure(&ring_buffer, "Failed flushing final exit batch to disk", &e);
            return Err(e);
        }
        Ok(())
    }

    //-----------------------------------------------------------------------------------------------

    /// Assigns monotonic sequence ID, dispatches console formatting, and stages file batch entry
    fn _stage_packet(
        mut packet: LogPacket,
        current_sequence: &mut u64,
        console_tx: &mpsc::Sender<String>,
        pending_batch: &mut Vec<String>,
        pending_entries: &mut Vec<LogEntry>,
    ) {
        packet.sequence = *current_sequence;
        *current_sequence += 1;

        let entry = packet.entry;
        let level_str = crate::models::log_entry::LEVEL_STRINGS
            .get(entry.level as usize)
            .unwrap_or(&"UNKNOWN");

        let console_msg = format_log_message(
            &entry.timestamp,
            &entry.hostname,
            &entry.logger_name,
            level_str,
            &entry.module,
            &entry.filename,
            &entry.function_name,
            &entry.line_number,
            &entry.message,
            &entry.path_name,
            &entry.process_id,
            &entry.process_name,
            &entry.thread_id,
            &entry.thread_name,
            &entry.service_name,
            &entry.stack_trace,
            true,
        );
        let _ = console_tx.try_send(console_msg);

        let file_msg = format_log_message(
            &entry.timestamp,
            &entry.hostname,
            &entry.logger_name,
            level_str,
            &entry.module,
            &entry.filename,
            &entry.function_name,
            &entry.line_number,
            &entry.message,
            &entry.path_name,
            &entry.process_id,
            &entry.process_name,
            &entry.thread_id,
            &entry.thread_name,
            &entry.service_name,
            &entry.stack_trace,
            false,
        );

        pending_batch.push(file_msg);
        pending_entries.push(entry);
    }

    //-----------------------------------------------------------------------------------------------

    /// Writes pending batch to disk, performs rotation check, flushes, and publishes to ring buffer
    async fn _flush_pending_batch(
        sink: &mut TransactionalFileWriter,
        pending_batch: &mut Vec<String>,
        pending_entries: &mut Vec<LogEntry>,
        config: &WriterConfig,
        ring_buffer: &crate::core::ring_buffer::LogRingBuffer,
    ) -> tokio::io::Result<()> {
        if let Err(e) = sink.write_batch(pending_batch).await {
            notify_disk_failure(
                ring_buffer,
                "Failed writing batch to disk after exhausting retries",
                &e,
            );
            return Err(e);
        }
        if let Err(e) = sink
            .check_and_rotate(config.max_file_bytes, config.backup_count)
            .await
        {
            notify_disk_failure(ring_buffer, "Failed file rotation after batch write", &e);
            return Err(e);
        }
        if let Err(e) = sink.flush().await {
            notify_disk_failure(ring_buffer, "Failed flushing batch to disk", &e);
            return Err(e);
        }

        pending_batch.clear();
        for entry in pending_entries.drain(..) {
            ring_buffer.push(entry);
        }

        Ok(())
    }
}

//-----------------------------------------------------------------------------------------------

/// Broadcast and record a fatal unrecoverable disk failure
fn notify_disk_failure(
    ring_buffer: &crate::core::ring_buffer::LogRingBuffer,
    context: &str,
    err: &std::io::Error,
) {
    let err_msg = format!("[DISK_FAILURE] {context}: {err}");
    let err_entry = LogEntry {
        timestamp: get_utc_timestamp(),
        hostname: get_hostname().to_string(),
        logger_name: "log_writer".to_string(),
        level: 11, // CRITICAL
        module: "log-server".to_string(),
        filename: "log_writer.rs".to_string(),
        function_name: "writer_task".to_string(),
        line_number: "disk_io".to_string(),
        message: err_msg.clone(),
        service_name: "log_server".to_string(),
        ..Default::default()
    };
    ring_buffer.push(err_entry);
    eprintln!("{err_msg}");
}

//-----------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rotate_files_cascading_and_pruning() {
        let test_dir = std::env::temp_dir().join(format!("log_server_test_rot_{}", std::process::id()));
        let _ = tokio::fs::create_dir_all(&test_dir).await;
        let base_path = test_dir.join("_main.log");

        // Cycle 1: First file
        tokio::fs::write(&base_path, b"entry 0").await.unwrap();
        LogWriter::rotate_files(&base_path, 3).await.unwrap();
        assert_eq!(
            tokio::fs::read_to_string(base_path.with_extension("log.0")).await.unwrap(),
            "entry 0"
        );

        // Cycle 2: Second file
        tokio::fs::write(&base_path, b"entry 1").await.unwrap();
        LogWriter::rotate_files(&base_path, 3).await.unwrap();
        assert_eq!(
            tokio::fs::read_to_string(base_path.with_extension("log.0")).await.unwrap(),
            "entry 1"
        );
        assert_eq!(
            tokio::fs::read_to_string(base_path.with_extension("log.1")).await.unwrap(),
            "entry 0"
        );

        // Cycle 3: Third file (fills backup_count = 3)
        tokio::fs::write(&base_path, b"entry 2").await.unwrap();
        LogWriter::rotate_files(&base_path, 3).await.unwrap();
        assert_eq!(
            tokio::fs::read_to_string(base_path.with_extension("log.0")).await.unwrap(),
            "entry 2"
        );
        assert_eq!(
            tokio::fs::read_to_string(base_path.with_extension("log.1")).await.unwrap(),
            "entry 1"
        );
        assert_eq!(
            tokio::fs::read_to_string(base_path.with_extension("log.2")).await.unwrap(),
            "entry 0"
        );

        // Cycle 4: Fourth file (oldest "entry 0" should be pruned)
        tokio::fs::write(&base_path, b"entry 3").await.unwrap();
        LogWriter::rotate_files(&base_path, 3).await.unwrap();
        assert_eq!(
            tokio::fs::read_to_string(base_path.with_extension("log.0")).await.unwrap(),
            "entry 3"
        );
        assert_eq!(
            tokio::fs::read_to_string(base_path.with_extension("log.1")).await.unwrap(),
            "entry 2"
        );
        assert_eq!(
            tokio::fs::read_to_string(base_path.with_extension("log.2")).await.unwrap(),
            "entry 1"
        );

        // Cleanup
        let _ = tokio::fs::remove_dir_all(&test_dir).await;
    }

    // -----------------------------------------------------------------------------

    #[tokio::test]
    async fn test_rotate_files_zero_backups() {
        let test_dir = std::env::temp_dir().join(format!("log_server_test_rot0_{}", std::process::id()));
        let _ = tokio::fs::create_dir_all(&test_dir).await;
        let base_path = test_dir.join("_main.log");

        tokio::fs::write(&base_path, b"temp data").await.unwrap();
        assert!(base_path.exists());

        LogWriter::rotate_files(&base_path, 0).await.unwrap();
        assert!(!base_path.exists());
        assert!(!base_path.with_extension("log.0").exists());

        let _ = tokio::fs::remove_dir_all(&test_dir).await;
    }

    // -----------------------------------------------------------------------------

    #[tokio::test]
    async fn test_writer_task_fatal_disk_failure_pushes_critical_and_terminates() {
        let test_dir = std::env::temp_dir().join(format!("log_server_test_fail_{}", std::process::id()));
        let _ = tokio::fs::create_dir_all(&test_dir).await;
        // Point base_file_path to an existing directory instead of a file
        // POSIX open(O_WRONLY) on a directory returns EISDIR (fatal I/O error)
        let invalid_file_path = test_dir.clone();

        let config = WriterConfig {
            max_retries: 1,
            retry_delay_ms: 10,
            ..WriterConfig::default()
        };

        let writer = LogWriter {
            config,
            base_file_path: invalid_file_path,
        };

        let ring_buffer = std::sync::Arc::new(crate::core::ring_buffer::LogRingBuffer::new(50));
        let (_tx, handle) = writer.start_writer_task(ring_buffer.clone());

        // Await the task handle; it should exit with Err
        let res = handle.await.expect("Task should complete, not panic");
        assert!(res.is_err(), "Writer task must fail on fatal disk error");

        // Verify ring buffer received structured [DISK_FAILURE] CRITICAL log
        let tail = ring_buffer.get_tail(10, None, None);
        assert!(
            tail.iter().any(|e| e.message.contains("[DISK_FAILURE]") && e.level == 11),
            "Ring buffer must contain [DISK_FAILURE] with level 11 (CRITICAL)"
        );

        let _ = tokio::fs::remove_dir_all(&test_dir).await;
    }
}
