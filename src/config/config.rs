//! ESSENTIAL PROCESS:
//! Log server configuration structure holding host, TCP socket port, gRPC port,
//! and dynamic hardware durability barrier (fsync) configuration.
//!
//! DATA FLOW:
//! 1. CLI / Environment / standalone.yaml -> Config::new() -> parse_fsync_config()
//! 2. Config injected into TCP and gRPC Server components and LogWriter
//!
//! KEY PARAMETERS:
//! - name: Service name (default "log_server").
//! - host: Network bind address for TCP server.
//! - port: TCP socket port.
//! - grpc_host: Network bind address for gRPC bridge.
//! - grpc_port: gRPC bridge port.
//! - sync_on_flush: Atomic hardware durability barrier (default false: OS page cache).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use microservice_toolbox::config::loader::AppConfig;

/// Server configuration
#[derive(Clone)]
pub struct Config {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub grpc_host: String,
    pub grpc_port: u16,
    pub sync_on_flush: Arc<AtomicBool>,
}

//-----------------------------------------------------------------------------------------------

impl Config {
    /// Create new server configuration
    pub fn new(name: &str, host: &str, port: u16, grpc_port: u16) -> Self {
        Self {
            name: name.to_string(),
            host: host.to_string(),
            port,
            grpc_host: host.to_string(),
            grpc_port,
            sync_on_flush: Arc::new(AtomicBool::new(false)),
        }
    }

    //-----------------------------------------------------------------------------------------------

    /// Override the gRPC bind host verbatim
    pub fn with_grpc_host(mut self, grpc_host: &str) -> Self {
        self.grpc_host = grpc_host.to_string();
        self
    }

    //-----------------------------------------------------------------------------------------------

    /// Configure hardware durability barrier (fsync)
    pub fn with_sync_on_flush(self, enabled: bool) -> Self {
        self.sync_on_flush.store(enabled, Ordering::SeqCst);
        self
    }

    //-----------------------------------------------------------------------------------------------

    /// Dynamic runtime toggle for hardware durability barrier (fsync)
    pub fn set_sync_on_flush(&self, enabled: bool) {
        self.sync_on_flush.store(enabled, Ordering::Relaxed);
    }

    //-----------------------------------------------------------------------------------------------

    /// Query current hardware durability barrier state
    pub fn is_sync_on_flush(&self) -> bool {
        self.sync_on_flush.load(Ordering::Relaxed)
    }
}

//-----------------------------------------------------------------------------------------------

/// Helper to parse fsync durability setting from AppConfig or environment
pub fn parse_fsync_config(ac: &AppConfig) -> bool {
    let capability = ac
        .cli_args
        .name
        .as_deref()
        .unwrap_or("log_server")
        .replace('-', "_");

    if let Some(val) = ac
        .get_value(&format!("capabilities.{capability}.fsync"))
        .or_else(|| ac.get_value("capabilities.log_server.fsync"))
    {
        if let Some(b) = val.as_bool() {
            return b;
        }
        if let Some(s) = val.as_str() {
            let s = s.trim().to_lowercase();
            return s == "true" || s == "1" || s == "yes";
        }
        if let Some(n) = val.as_i64() {
            return n == 1;
        }
    }

    if let Ok(env_val) = std::env::var("LS_FSYNC") {
        let s = env_val.trim().to_lowercase();
        return s == "true" || s == "1" || s == "yes";
    }

    false
}

//-----------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_new() {
        let config = Config::new("test-server", "127.0.0.1", 9000, 9001);
        assert_eq!(config.name, "test-server");
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 9000);
        assert_eq!(config.grpc_host, "127.0.0.1");
        assert_eq!(config.grpc_port, 9001);
        assert!(!config.is_sync_on_flush());
    }

    #[test]
    fn test_config_with_grpc_host() {
        let config = Config::new("test-server", "127.0.0.1", 9000, 9001).with_grpc_host("log-server");
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.grpc_host, "log-server");
        assert_eq!(config.grpc_port, 9001);
    }

    #[test]
    fn test_config_fsync_toggle_on_the_fly() {
        let config = Config::new("test-server", "127.0.0.1", 9000, 9001);
        assert!(!config.is_sync_on_flush());

        // Toggle on the fly
        config.set_sync_on_flush(true);
        assert!(config.is_sync_on_flush());

        // Cloned config shares the same atomic state
        let cloned = config.clone();
        assert!(cloned.is_sync_on_flush());

        cloned.set_sync_on_flush(false);
        assert!(!config.is_sync_on_flush());
    }

    #[test]
    fn test_parse_fsync_config_defaults() {
        let ac_empty = AppConfig::default();
        assert!(!parse_fsync_config(&ac_empty));
    }
}
