//! ESSENTIAL PROCESS:
//! Log server configuration structure holding host, TCP socket port, and gRPC port.
//!
//! DATA FLOW:
//! 1. CLI / Environment / standalone.yaml -> Config::new()
//! 2. Config injected into TCP and gRPC Server components
//!
//! KEY PARAMETERS:
//! - name: Service name (default "log_server").
//! - host: Network bind address (default 0.0.0.0).
//! - port: TCP socket port (default 9020).
//! - grpc_port: gRPC bridge port (default 9021).

/// Server configuration
#[derive(Clone)]
pub struct Config {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub grpc_port: u16,
}

//-----------------------------------------------------------------------------------------------

impl Config {
    /// Create new server configuration
    pub fn new(name: &str, host: &str, port: u16, grpc_port: u16) -> Self {
        Self {
            name: name.to_string(),
            host: host.to_string(),
            port,
            grpc_port,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_new() {
        let config = Config::new("test-server", "127.0.0.1", 9000, 9001);
        assert_eq!(config.name, "test-server");
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 9000);
        assert_eq!(config.grpc_port, 9001);
    }
}
