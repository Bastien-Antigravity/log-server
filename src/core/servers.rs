//! Main log server orchestrator
//!
//! Coordinates TCP and gRPC servers with shared file writer.

use std::sync::Arc;

use crate::network::tcp_server::TcpServer;
use crate::network::grpc_server::GrpcServer;
use crate::core::writers::LogWriter;
use crate::common::config::ServerConfig;



 
/// Main log server orchestrator
pub struct LogServer {
    name: String,
    config: ServerConfig,
    writer: Arc<LogWriter>,
    enable_grpc: bool,
}

//-----------------------------------------------------------------------------------------------

impl LogServer {
    /// Create new log server instance
    pub async fn new(name: &str, host: &str, port: u16, grpc_port: u16, enable_grpc: bool) -> Result<Self, Box<dyn std::error::Error>> {
        let config = ServerConfig::new(name, host, port, grpc_port);
        
        // Create log directory
        crate::utils::create_log_folder("logs")?;
        
        // Initialize writer
        let writer = Arc::new(LogWriter::new().await?);
        
        Ok(Self {
            name: name.to_string(),
            config,
            writer,
            enable_grpc,
        })
    }
    
    //-----------------------------------------------------------------------------------------------
    
    /// Run the log server with all components
    pub async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        println!("{} : starting server components", self.name);
        
        
        // Start TCP server (always)
        let tcp_server = TcpServer::new(&self.config, self.writer.clone());
        let tcp_handle = tokio::spawn(async move {
            if let Err(e) = tcp_server.run().await {
                eprintln!("TCP server error: {}", e);
            }
        });
        
        // Conditionally start gRPC server
        let grpc_handle = if self.enable_grpc {
            let grpc_server = GrpcServer::new(&self.config, self.writer.clone());
            Some(tokio::spawn(async move {
                if let Err(e) = grpc_server.run().await {
                    eprintln!("gRPC server error: {}", e);
                }
            }))
        } else {
            None
        };
        
        println!("{} : all server components started", self.name);
        
        // Wait for servers to complete
        let _ = tcp_handle.await;
        
        if let Some(grpc_handle) = grpc_handle {
        let _ = grpc_handle.await;
        }
        
        Ok(())
    }
}