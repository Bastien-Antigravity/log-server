//! File writer with ordering and rotation
//!
//! Handles ordered writing of log messages to files with rotation.

use crate::core::log_formatter::format_log_message;
use crate::models::log_packet::LogPacket;
use std::collections::BTreeMap;
use std::path::PathBuf;
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
    sync::mpsc,
    time::{sleep, Duration},
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
}

//-----------------------------------------------------------------------------------------------

impl Default for WriterConfig {
    fn default() -> Self {
        Self {
            initial_batch_size: 100,
            buffer_size: 1024,
            max_retries: 3,
            retry_delay_ms: 100,
            max_file_bytes: 1024 * 1024, // 1 MB
            backup_count: 10,
            gap_timeout_ms: 500,
        }
    }
}

//-----------------------------------------------------------------------------------------------

/// File writer with ordering and rotation
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

    /// Create new log writer with custom config
    pub async fn with_config(config: WriterConfig) -> Result<Self, std::io::Error> {
        let log_dir = crate::utils::helpers::get_exec_parent_dir().join("logs");

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

    /// Start the writer task
    pub fn start_writer_task(&self) -> mpsc::Sender<LogPacket> {
        let (writer_tx, writer_rx) = mpsc::channel::<LogPacket>(self.config.buffer_size);
        let base_path = self.base_file_path.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            if let Err(e) = Self::writer_task(writer_rx, base_path, config).await {
                eprintln!("Writer task failed: {e}");
            }
        });

        writer_tx
    }

    //-----------------------------------------------------------------------------------------------

    /// Main writer task implementation
    async fn writer_task(
        mut rx: mpsc::Receiver<LogPacket>,
        base_file_path: PathBuf,
        config: WriterConfig,
    ) -> tokio::io::Result<()> {
        let mut file = File::create(&base_file_path).await?;
        let mut file_size = 0u64;
        let mut buffer: BTreeMap<u64, crate::models::log_entry::LogEntry> = BTreeMap::new();
        let mut current_sequence: u64 = 0;
        let mut batch_size = config.initial_batch_size;
        let mut gap_timer = tokio::time::interval(Duration::from_millis(config.gap_timeout_ms));

        loop {
            tokio::select! {
                // Branch 1: Incoming structured packets (Zero-String Policy)
                Some(packet) = rx.recv() => {
                    buffer.insert(packet.sequence, packet.entry);
                }

                // Branch 2: Gap Timeout / Periodic Flush
                _ = gap_timer.tick() => {
                    if !buffer.is_empty() && !buffer.contains_key(&current_sequence) {
                        let next_available = *buffer.keys().next().unwrap();
                        if next_available > current_sequence {
                            eprintln!("[SEQUENCE_GAP_WARNING] Skipping from {} to {} due to timeout",
                                current_sequence, next_available);
                            current_sequence = next_available;
                        }
                    }
                }

                // Shutdown condition
                else => break,
            }

            // Process buffer if current_sequence is ready or buffer is too full
            while buffer.contains_key(&current_sequence) || buffer.len() >= batch_size {
                let mut batch = Vec::new();

                // If we hit batch_size but don't have current_sequence, we must force progress
                if !buffer.contains_key(&current_sequence) && buffer.len() >= batch_size {
                    let next_available = *buffer.keys().next().unwrap();
                    eprintln!(
                        "[BUFFER_FULL_WARNING] Forcing progress to {} due to buffer pressure",
                        next_available
                    );
                    current_sequence = next_available;
                }

                while let Some(entry) = buffer.remove(&current_sequence) {
                    // Format for console (with colors)
                    let console_msg = format_log_message(
                        &entry.timestamp,
                        &entry.hostname,
                        &entry.logger_name,
                        crate::models::log_entry::LEVEL_STRINGS[entry.level as usize],
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
                    println!("{console_msg}");

                    // Format for file (no colors)
                    let file_msg = format_log_message(
                        &entry.timestamp,
                        &entry.hostname,
                        &entry.logger_name,
                        crate::models::log_entry::LEVEL_STRINGS[entry.level as usize],
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

                    batch.push(file_msg);
                    current_sequence += 1;

                    if batch.len() >= batch_size {
                        break;
                    }
                }

                if !batch.is_empty() {
                    Self::write_batch(&mut file, &mut file_size, &batch, &config).await?;

                    if file_size >= config.max_file_bytes {
                        file.flush().await?;
                        Self::rotate_files(&base_file_path, config.backup_count).await?;
                        file = File::create(&base_file_path).await?;
                        file_size = 0;
                    }
                    file.flush().await?;
                }

                if buffer.is_empty() {
                    break;
                }
            }

            // Adjust batch size dynamically
            if buffer.len() > batch_size {
                batch_size = (batch_size * 2).min(1000);
            } else if buffer.len() < batch_size / 2 {
                batch_size = (batch_size / 2).max(10);
            }
        }

        // Flush remaining messages
        for (_, entry) in buffer {
            let file_msg = format_log_message(
                &entry.timestamp,
                &entry.hostname,
                &entry.logger_name,
                crate::models::log_entry::LEVEL_STRINGS[entry.level as usize],
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
            let log_line = format!("{file_msg}\n");
            file.write_all(log_line.as_bytes()).await?;
        }

        file.flush().await?;
        Ok(())
    }

    //-----------------------------------------------------------------------------------------------

    /// Write batch with retry logic
    async fn write_batch(
        file: &mut File,
        file_size: &mut u64,
        batch: &[String],
        config: &WriterConfig,
    ) -> tokio::io::Result<()> {
        for attempt in 0..=config.max_retries {
            let mut success = true;
            for data in batch {
                let log_entry = format!("{data}\n");
                if file.write_all(log_entry.as_bytes()).await.is_err() {
                    success = false;
                    break;
                }
                *file_size += data.len() as u64;
            }

            if success {
                break;
            } else if attempt < config.max_retries {
                sleep(Duration::from_millis(config.retry_delay_ms)).await;
            } else {
                return Err(std::io::Error::other("Write failed after maximum retries"));
            }
        }
        Ok(())
    }

    //-----------------------------------------------------------------------------------------------

    /// Rotate log files
    async fn rotate_files(base_path: &PathBuf, backup_count: usize) -> tokio::io::Result<()> {
        for i in (1..=backup_count).rev() {
            let old_path = base_path.with_extension(format!("log.{}", i - 1));
            let new_path = base_path.with_extension(format!("log.{i}"));

            if fs::metadata(&old_path).await.is_ok() {
                fs::rename(&old_path, &new_path).await?;
            }
        }

        fs::rename(&base_path, &base_path.with_extension("log.0")).await?;
        Ok(())
    }
}
