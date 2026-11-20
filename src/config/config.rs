//! ESSENTIAL PROCESS:
//! Log server configuration structure holding host, TCP socket port, and gRPC port.
//!
//! DATA FLOW:
//! 1. CLI / Environment / standalone.yaml -> Config::new()
//! 2. Config injected into TCP and gRPC Server components
//!
//! KEY PARAMETERS:
//! - name: Service name (default "log_server").
//! - host: Network bind address for TCP server.
//! - port: TCP socket port.
//! - grpc_host: Network bind address for gRPC bridge.
//! - grpc_port: gRPC bridge port.

/// Server configuration
#[derive(Clone)]
pub struct Config {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub grpc_host: String,
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
            grpc_host: host.to_string(),
            grpc_port,
        }
    }

    //-----------------------------------------------------------------------------------------------

    /// Override the gRPC bind host verbatim
    pub fn with_grpc_host(mut self, grpc_host: &str) -> Self {
        self.grpc_host = grpc_host.to_string();
        self
    }
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
    }

    #[test]
    fn test_config_with_grpc_host() {
        let config = Config::new("test-server", "127.0.0.1", 9000, 9001).with_grpc_host("log-server");
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.grpc_host, "log-server");
        assert_eq!(config.grpc_port, 9001);
    }
}
