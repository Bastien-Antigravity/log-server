//! ESSENTIAL PROCESS:
//! Transactional disk persistence engine with in-memory batch staging (Tier 1),
//! checkpointed ftruncate rollback on partial write errors (Tier 2),
//! and poisoned file handle recovery (Tier 3).
//!
//! DATA FLOW:
//! 1. Formatted log entry batch -> In-memory contiguous staging buffer (Vec<u8>)
//! 2. Capture clean record boundary checkpoint (*file_size)
//! 3. Atomic write_all() execution to file descriptor
//! 4. On write error: invoke ftruncate to checkpoint_offset to drop partial bytes
//! 5. On rollback failure: poison handle, close descriptor, reopen, and resynchronize
//!
//! KEY PARAMETERS:
//! - max_retries: Number of retry attempts on transient write failures.
//! - retry_delay_ms: Backoff duration between retries in milliseconds.
// cli:allow-print
//! - sync_on_flush: Dynamic atomic hardware durability barrier (fsync).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{AsyncSeekExt, AsyncWriteExt, SeekFrom};
use tokio::time::sleep;

/// Transactional log file manager providing atomic batch commits and rollback
pub struct TransactionalFileWriter {
    base_file_path: PathBuf,
    file: File,
    file_size: u64,
    max_retries: usize,
    retry_delay_ms: u64,
    sync_on_flush: Arc<AtomicBool>,
}

// -----------------------------------------------------------------------------

impl TransactionalFileWriter {
    /// Initialize a new transactional file writer for the given base path with default OS page-cache flush
    pub async fn new(
        base_file_path: PathBuf,
        max_retries: usize,
        retry_delay_ms: u64,
    ) -> tokio::io::Result<Self> {
        Self::with_sync(
            base_file_path,
            max_retries,
            retry_delay_ms,
            Arc::new(AtomicBool::new(false)),
        )
        .await
    }

    // -----------------------------------------------------------------------------

    /// Initialize a transactional file writer with a shared hardware durability barrier atomic
    pub async fn with_sync(
        base_file_path: PathBuf,
        max_retries: usize,
        retry_delay_ms: u64,
        sync_on_flush: Arc<AtomicBool>,
    ) -> tokio::io::Result<Self> {
        if let Some(parent) = base_file_path.parent() {
            if let Some(parent_str) = parent.to_str() {
                crate::utils::helpers::create_log_folder(parent_str)?;
            }
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&base_file_path)
            .await?;
        let file_size = file.metadata().await?.len();

        Ok(Self {
            base_file_path,
            file,
            file_size,
            max_retries,
            retry_delay_ms,
            sync_on_flush,
        })
    }

    // -----------------------------------------------------------------------------

    /// Write a batch of formatted log lines transactionally with 3-tier recovery:
    /// - Tier 1: In-memory contiguous staging buffer (1 syscall per batch).
    /// - Tier 2: Checkpoint offset tracking & ftruncate rollback on partial write.
    /// - Tier 3: Poisoned handle recovery (close and reopen) if rollback fails.
    pub async fn write_batch(&mut self, batch: &[String]) -> tokio::io::Result<()> {
        if batch.is_empty() {
            return Ok(());
        }

        // Tier 1: In-memory batch staging into a single contiguous buffer
        let total_bytes: usize = batch.iter().map(|s| s.len() + 1).sum();
        let mut staged = Vec::with_capacity(total_bytes);
        for line in batch {
            staged.extend_from_slice(line.as_bytes());
            staged.push(b'\n');
        }

        let checkpoint_offset = self.file_size;

        for attempt in 0..=self.max_retries {
            match self.file.write_all(&staged).await {
                Ok(()) => {
                    self.file_size += staged.len() as u64;
                    return Ok(());
                }
                Err(write_err) => {
                    eprintln!(
                        "Partial write failure on attempt {}/{}: {write_err}. Executing ftruncate rollback to {checkpoint_offset}.",
                        attempt + 1,
                        self.max_retries + 1
                    );

                    // Tier 2: Truncation rollback to clean record boundary
                    match self.rollback_to(checkpoint_offset).await {
                        Ok(()) => {
                            if attempt < self.max_retries {
                                sleep(Duration::from_millis(self.retry_delay_ms)).await;
                                continue;
                            } else {
                                return Err(write_err);
                            }
                        }
                        Err(rollback_err) => {
                            // Tier 3: Truncate failed (e.g. EIO or EROFS). Handle is poisoned.
                            eprintln!(
                                "CRITICAL: Rollback failed: {rollback_err}. Poisoning handle and executing emergency reopen."
                            );
                            self.recover_poisoned_handle(checkpoint_offset).await?;
                            return Err(write_err);
                        }
                    }
                }
            }
        }

        Err(std::io::Error::other("Write failed after maximum retries"))
    }

    //-----------------------------------------------------------------------------------------------

    /// Roll back the file length and cursor to the specified clean checkpoint offset
    pub async fn rollback_to(&mut self, checkpoint_offset: u64) -> tokio::io::Result<()> {
        self.file.set_len(checkpoint_offset).await?;
        self.file.seek(SeekFrom::End(0)).await?;
        self.file_size = checkpoint_offset;
        Ok(())
    }

    //-----------------------------------------------------------------------------------------------

    /// Tier 3: Recovers from a poisoned file descriptor when truncation fails.
    /// Drops the current corrupted handle, opens a fresh descriptor, and synchronizes metadata.
    async fn recover_poisoned_handle(&mut self, checkpoint_offset: u64) -> tokio::io::Result<()> {
        let fresh_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.base_file_path)
            .await?;

        self.file = fresh_file;
        let actual_len = self.file.metadata().await?.len();

        // If file expanded beyond checkpoint due to the partial write, truncate on fresh descriptor
        if actual_len > checkpoint_offset {
            self.file.set_len(checkpoint_offset).await?;
            self.file.seek(SeekFrom::End(0)).await?;
        }

        self.file_size = self.file.metadata().await?.len();
        if self.file_size != checkpoint_offset {
            return Err(std::io::Error::other(format!(
                "Failed to restore clean record boundary after handle recovery: expected offset {}, found {}",
                checkpoint_offset, self.file_size
            )));
        }
        Ok(())
    }

    //-----------------------------------------------------------------------------------------------

    /// Check if rotation threshold is met and perform cascading rotation
    pub async fn check_and_rotate(
        &mut self,
        max_file_bytes: u64,
        backup_count: usize,
    ) -> tokio::io::Result<bool> {
        if self.file_size < max_file_bytes {
            return Ok(false);
        }

        self.file.flush().await?;
        Self::rotate_files(&self.base_file_path, backup_count).await?;

        // Open fresh file after rotation
        self.file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.base_file_path)
            .await?;
        self.file_size = 0;

        Ok(true)
    }

    // -----------------------------------------------------------------------------

    /// Cascading file rotation implementation
    pub async fn rotate_files(base_path: &PathBuf, backup_count: usize) -> tokio::io::Result<()> {
        if backup_count == 0 {
            let _ = fs::remove_file(base_path).await;
            return Ok(());
        }

        // Delete the oldest backup file if it exists to preserve exact backup_count
        let oldest = base_path.with_extension(format!("log.{}", backup_count - 1));
        if fs::metadata(&oldest).await.is_ok() {
            let _ = fs::remove_file(&oldest).await;
        }

        for i in (1..backup_count).rev() {
            let old_path = base_path.with_extension(format!("log.{}", i - 1));
            let new_path = base_path.with_extension(format!("log.{i}"));

            if fs::metadata(&old_path).await.is_ok() {
                fs::rename(&old_path, &new_path).await?;
            }
        }

        if fs::metadata(base_path).await.is_ok() {
            fs::rename(base_path, base_path.with_extension("log.0")).await?;
        }
        Ok(())
    }

    // -----------------------------------------------------------------------------

    /// Flush the active file to disk, optionally executing hardware durability commit (fsync/fdatasync).
    pub async fn flush(&mut self) -> tokio::io::Result<()> {
        self.file.flush().await?;
        if self.sync_on_flush.load(Ordering::Relaxed) {
            self.file.sync_data().await?;
        }
        Ok(())
    }

    // -----------------------------------------------------------------------------

    /// Dynamically toggle hardware durability barrier (fsync) on the fly.
    pub fn set_sync_on_flush(&self, enabled: bool) {
        self.sync_on_flush.store(enabled, Ordering::Relaxed);
    }

    // -----------------------------------------------------------------------------

    /// Query current hardware durability barrier state.
    pub fn is_sync_on_flush(&self) -> bool {
        self.sync_on_flush.load(Ordering::Relaxed)
    }

    // -----------------------------------------------------------------------------

    /// Access the underlying shared atomic boolean.
    pub fn sync_on_flush_handle(&self) -> Arc<AtomicBool> {
        self.sync_on_flush.clone()
    }

    // -----------------------------------------------------------------------------

    /// Return current tracked file size
    pub fn file_size(&self) -> u64 {
        self.file_size
    }
}

// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_transactional_writer_sync_on_flush_toggled_on_the_fly() {
        let test_dir = std::env::temp_dir().join(format!("test_txn_fsync_{}", std::process::id()));
        let _ = fs::create_dir_all(&test_dir).await;
        let base_path = test_dir.join("_main.log");

        let sync_flag = Arc::new(AtomicBool::new(false));
        let mut writer = TransactionalFileWriter::with_sync(base_path.clone(), 3, 50, sync_flag.clone())
            .await
            .unwrap();

        assert!(!writer.is_sync_on_flush());

        // 1. Flush with fsync = false (OS page cache)
        writer.write_batch(&["stage1".to_string()]).await.unwrap();
        writer.flush().await.unwrap();

        // 2. Toggle on the fly: fsync = true (hardware NAND commit)
        writer.set_sync_on_flush(true);
        assert!(writer.is_sync_on_flush());
        assert!(sync_flag.load(Ordering::Relaxed));

        writer.write_batch(&["stage2".to_string()]).await.unwrap();
        writer.flush().await.unwrap();

        // 3. Toggle back on the fly: fsync = false
        sync_flag.store(false, Ordering::Relaxed);
        assert!(!writer.is_sync_on_flush());

        writer.write_batch(&["stage3".to_string()]).await.unwrap();
        writer.flush().await.unwrap();

        let content = fs::read_to_string(&base_path).await.unwrap();
        assert_eq!(content, "stage1\nstage2\nstage3\n");

        let _ = fs::remove_dir_all(&test_dir).await;
    }

    // -----------------------------------------------------------------------------

    #[tokio::test]
    async fn test_transactional_write_and_flush() {
        let test_dir = std::env::temp_dir().join(format!("test_txn_write_{}", std::process::id()));
        let _ = fs::create_dir_all(&test_dir).await;
        let base_path = test_dir.join("_main.log");

        let mut writer = TransactionalFileWriter::new(base_path.clone(), 3, 50)
            .await
            .unwrap();

        let batch = vec!["line_alpha".to_string(), "line_beta".to_string()];
        writer.write_batch(&batch).await.unwrap();
        writer.flush().await.unwrap();

        let content = fs::read_to_string(&base_path).await.unwrap();
        assert_eq!(content, "line_alpha\nline_beta\n");
        assert_eq!(writer.file_size(), content.len() as u64);

        let _ = fs::remove_dir_all(&test_dir).await;
    }

    // -----------------------------------------------------------------------------

    #[tokio::test]
    async fn test_transactional_write_truncation_rollback() {
        let test_dir = std::env::temp_dir().join(format!("test_txn_rollback_{}", std::process::id()));
        let _ = fs::create_dir_all(&test_dir).await;
        let base_path = test_dir.join("_main.log");

        let mut writer = TransactionalFileWriter::new(base_path.clone(), 3, 50)
            .await
            .unwrap();

        // 1. Initial clean batch
        writer.write_batch(&["clean_record_1".to_string()]).await.unwrap();
        writer.flush().await.unwrap();
        let checkpoint = writer.file_size();

        // 2. Simulate partial write leakage directly to the underlying file
        writer.file.write_all(b"PARTIAL_GARBAGE_FRAGMENT").await.unwrap();
        writer.file.flush().await.unwrap();
        let corrupt_content = fs::read_to_string(&base_path).await.unwrap();
        assert!(corrupt_content.contains("PARTIAL_GARBAGE_FRAGMENT"));

        // 3. Trigger truncation rollback
        writer.rollback_to(checkpoint).await.unwrap();

        // 4. Verify garbage is physically truncated from disk
        let rolled_back_content = fs::read_to_string(&base_path).await.unwrap();
        assert!(!rolled_back_content.contains("PARTIAL_GARBAGE_FRAGMENT"));
        assert_eq!(rolled_back_content, "clean_record_1\n");
        assert_eq!(writer.file_size(), checkpoint);

        // 5. Subsequent write succeeds cleanly at record boundary without prefix duplication
        writer.write_batch(&["clean_record_2".to_string()]).await.unwrap();
        writer.flush().await.unwrap();

        let final_content = fs::read_to_string(&base_path).await.unwrap();
        assert_eq!(final_content, "clean_record_1\nclean_record_2\n");
        assert!(!final_content.contains("PARTIAL_GARBAGE_FRAGMENT"));

        let _ = fs::remove_dir_all(&test_dir).await;
    }

    // -----------------------------------------------------------------------------

    #[tokio::test]
    async fn test_transactional_rotation_cascading() {
        let test_dir = std::env::temp_dir().join(format!("test_txn_rot_{}", std::process::id()));
        let _ = fs::create_dir_all(&test_dir).await;
        let base_path = test_dir.join("_main.log");

        let mut writer = TransactionalFileWriter::new(base_path.clone(), 3, 50)
            .await
            .unwrap();

        writer.write_batch(&["batch_0".to_string()]).await.unwrap();
        let rotated = writer.check_and_rotate(1, 2).await.unwrap();
        assert!(rotated, "Rotation threshold should be triggered");
        assert_eq!(writer.file_size(), 0);

        assert!(base_path.with_extension("log.0").exists());

        let _ = fs::remove_dir_all(&test_dir).await;
    }

    //-----------------------------------------------------------------------------------------------

    #[tokio::test]
    async fn test_recover_poisoned_handle_restores_checkpoint_boundary() {
        let test_dir = std::env::temp_dir().join(format!("test_txn_recover_{}", std::process::id()));
        let _ = fs::create_dir_all(&test_dir).await;
        let base_path = test_dir.join("_main.log");

        let mut writer = TransactionalFileWriter::new(base_path.clone(), 2, 10)
            .await
            .unwrap();

        writer.write_batch(&["record_alpha".to_string()]).await.unwrap();
        writer.flush().await.unwrap();
        let checkpoint = writer.file_size();

        // Simulate partial write beyond checkpoint before handle recovery
        writer.file.write_all(b"CORRUPT_TAIL").await.unwrap();
        writer.file.flush().await.unwrap();

        writer.recover_poisoned_handle(checkpoint).await.unwrap();
        assert_eq!(writer.file_size(), checkpoint);

        let content = fs::read_to_string(&base_path).await.unwrap();
        assert_eq!(content, "record_alpha\n");

        // Verify fails if checkpoint is unreachable (e.g. file shorter than expected checkpoint)
        let err = writer.recover_poisoned_handle(checkpoint + 100).await;
        assert!(err.is_err(), "Mismatch between recovered file size and checkpoint must return error");

        let _ = fs::remove_dir_all(&test_dir).await;
    }
}

