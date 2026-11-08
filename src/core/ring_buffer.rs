//! ESSENTIAL PROCESS:
//! In-memory circular ring buffer for real-time log ingestion, querying,
//! and zero-copy broadcast streaming via gRPC.
//!
//! DATA FLOW:
//! 1. LogWriter -> push(entry) -> VecDeque (bounded FIFO memory)
//! 2. Real-time broadcast -> broadcast_tx -> active gRPC StreamLogs subscribers
//! 3. GetTailLogs -> thread-safe RwLock read -> filtered recent log snapshot
//!
//! KEY PARAMETERS:
//! - capacity: Maximum entries retained in memory before FIFO rotation (default 5,000).
//! - broadcast_tx: Tokio broadcast channel for real-time streaming subscribers (capacity 4,096).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;
use tokio::sync::broadcast;

use crate::models::log_entry::LogEntry;

/// Thread-safe in-memory circular ring buffer with real-time broadcast
pub struct LogRingBuffer {
    capacity: usize,
    buffer: RwLock<VecDeque<LogEntry>>,
    total_ingested: AtomicU64,
    broadcast_tx: broadcast::Sender<LogEntry>,
}

//-----------------------------------------------------------------------------------------------

impl LogRingBuffer {
    /// Create new ring buffer with given capacity and internal broadcast channel
    pub fn new(capacity: usize) -> Self {
        let (broadcast_tx, _) = broadcast::channel(4096);
        Self {
            capacity: if capacity == 0 { 5000 } else { capacity },
            buffer: RwLock::new(VecDeque::with_capacity(capacity)),
            total_ingested: AtomicU64::new(0),
            broadcast_tx,
        }
    }

    //-----------------------------------------------------------------------------------------------

    /// Push a new log entry into the circular buffer and broadcast to live subscribers
    pub fn push(&self, entry: LogEntry) {
        self.total_ingested.fetch_add(1, Ordering::Relaxed);

        // Real-time broadcast: send to any active gRPC streams (ignore Err when no listeners)
        let _ = self.broadcast_tx.send(entry.clone());

        if let Ok(mut lock) = self.buffer.write() {
            if lock.len() >= self.capacity {
                lock.pop_front();
            }
            lock.push_back(entry);
        }
    }

    //-----------------------------------------------------------------------------------------------

    /// Subscribe to real-time log stream
    pub fn subscribe(&self) -> broadcast::Receiver<LogEntry> {
        self.broadcast_tx.subscribe()
    }

    //-----------------------------------------------------------------------------------------------

    /// Retrieve the most recent entries up to `limit`, with optional filters
    pub fn get_tail(
        &self,
        limit: usize,
        service_filter: Option<&str>,
        min_level: Option<i32>,
    ) -> Vec<LogEntry> {
        let lock = match self.buffer.read() {
            Ok(guard) => guard,
            Err(_) => return Vec::new(),
        };

        let effective_limit = if limit == 0 { 100 } else { limit.min(self.capacity) };

        lock.iter()
            .rev()
            .filter(|e| {
                if let Some(svc) = service_filter {
                    if !svc.is_empty() && e.service_name != svc {
                        return false;
                    }
                }
                if let Some(min_lvl) = min_level {
                    if e.level < min_lvl {
                        return false;
                    }
                }
                true
            })
            .take(effective_limit)
            .cloned()
            .collect::<Vec<LogEntry>>()
            .into_iter()
            .rev()
            .collect()
    }

    //-----------------------------------------------------------------------------------------------

    /// Return total number of logs ingested since process start
    pub fn total_ingested(&self) -> u64 {
        self.total_ingested.load(Ordering::Relaxed)
    }
}

//-----------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_capacity_and_fifo() {
        let ring = LogRingBuffer::new(3);
        for i in 1..=5 {
            ring.push(LogEntry {
                message: format!("msg {i}"),
                service_name: "test_service".to_string(),
                level: 3,
                ..Default::default()
            });
        }

        assert_eq!(ring.total_ingested(), 5);
        let tail = ring.get_tail(10, None, None);
        assert_eq!(tail.len(), 3);
        assert_eq!(tail[0].message, "msg 3");
        assert_eq!(tail[1].message, "msg 4");
        assert_eq!(tail[2].message, "msg 5");
    }

    #[test]
    fn test_ring_buffer_filters() {
        let ring = LogRingBuffer::new(10);
        ring.push(LogEntry {
            message: "info rag".to_string(),
            service_name: "rag_engine".to_string(),
            level: 3,
            ..Default::default()
        });
        ring.push(LogEntry {
            message: "error rag".to_string(),
            service_name: "rag_engine".to_string(),
            level: 10,
            ..Default::default()
        });
        ring.push(LogEntry {
            message: "info squad".to_string(),
            service_name: "agent_squad".to_string(),
            level: 3,
            ..Default::default()
        });

        let filtered_svc = ring.get_tail(10, Some("rag_engine"), None);
        assert_eq!(filtered_svc.len(), 2);

        let filtered_lvl = ring.get_tail(10, None, Some(10));
        assert_eq!(filtered_lvl.len(), 1);
        assert_eq!(filtered_lvl[0].message, "error rag");
    }

    #[tokio::test]
    async fn test_ring_buffer_broadcast() {
        let ring = LogRingBuffer::new(10);
        let mut rx = ring.subscribe();

        ring.push(LogEntry {
            message: "live broadcast message".to_string(),
            service_name: "live_test".to_string(),
            level: 3,
            ..Default::default()
        });

        let received = rx.recv().await.expect("Failed to receive broadcast");
        assert_eq!(received.message, "live broadcast message");
        assert_eq!(received.service_name, "live_test");
    }
}

