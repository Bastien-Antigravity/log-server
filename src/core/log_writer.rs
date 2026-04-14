//! File writer with ordering and rotation
//!
//! Handles ordered writing of log messages to files with rotation.

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
        }
    }
}

//-----------------------------------------------------------------------------------------------

/// File writer with ordering and rotation
pub struct LogWriter {
    config: WriterConfig,
    base_file_path: PathBuf,
}

//-----------------------------------------------------------------------------------------------

impl LogWriter {
    /// Create new log writer
    pub async fn new() -> Result<Self, std::io::Error> {
        let log_dir = crate::utils::helpers::get_exec_parent_dir().join("logs");

        // Ensure log directory exists
        if let Some(log_dir_str) = log_dir.to_str() {
            crate::utils::helpers::create_log_folder(log_dir_str)?;
        }

        let base_file_path = log_dir.join("_main.log");

        Ok(Self {
            config: WriterConfig::default(),
            base_file_path,
        })
    }

    //-----------------------------------------------------------------------------------------------

    /// Start the writer task
    pub fn start_writer_task(&self) -> mpsc::Sender<String> {
        let (writer_tx, writer_rx) = mpsc::channel::<String>(self.config.buffer_size);
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
        mut rx: mpsc::Receiver<String>,
        base_file_path: PathBuf,
        config: WriterConfig,
    ) -> tokio::io::Result<()> {
        let mut file = File::create(&base_file_path).await?;
        let mut file_size = 0u64;
        let mut buffer: BTreeMap<u64, String> = BTreeMap::new();
        let mut current_sequence: u64 = 0;
        let mut batch_size = config.initial_batch_size;

        while let Some(message) = rx.recv().await {
            // Parse sequence number and message
            if let Some((seq_str, log_data)) = message.split_once(' ') {
                if let Ok(sequence) = seq_str.parse::<u64>() {
                    buffer.insert(sequence, log_data.to_string());
                }
            }

            // Process batch if ready
            while buffer.len() >= batch_size || buffer.contains_key(&current_sequence) {
                let mut batch = Vec::new();

                for _ in 0..batch_size {
                    if let Some(data) = buffer.remove(&current_sequence) {
                        // Threshold updated to 80 (pre-level 70 + level 10)
                        if data.len() >= 80 {
                            let level_part = data[70..80].trim();
                            let colored_level =
                                crate::utils::terminal_ui::colorize_level(level_part);
                            // Print to console with colors
                            println!("{}{}{}", &data[..70], colored_level, &data[80..]);
                        } else {
                            println!("{data}");
                        }

                        batch.push(data);
                        current_sequence += 1;
                    } else {
                        break;
                    }
                }

                if !batch.is_empty() {
                    Self::write_batch(&mut file, &mut file_size, &batch, &config).await?;

                    // Rotate file if size exceeds limit
                    if file_size >= config.max_file_bytes {
                        file.flush().await?;
                        Self::rotate_files(&base_file_path, config.backup_count).await?;
                        file = File::create(&base_file_path).await?;
                        file_size = 0;
                    }

                    file.flush().await?;
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
        for (_, data) in buffer {
            let log_entry = format!("{data}\n");
            file.write_all(log_entry.as_bytes()).await?;
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
