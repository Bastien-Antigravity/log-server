//! Server configuration




/// Server configuration
#[derive(Clone)]
pub struct ServerConfig {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub grpc_port: u16,
}

//-----------------------------------------------------------------------------------------------

impl ServerConfig {
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