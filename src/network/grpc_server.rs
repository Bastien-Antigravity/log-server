//! gRPC server for log messages
//!
//! Provides gRPC endpoint for receiving log messages alongside TCP socket.

use tonic::{transport::Server, Request, Response, Status};
use tokio::sync::mpsc;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use crate::common::config::ServerConfig;
use crate::core::writers::LogWriter;
use crate::core::handlers::handle_grpc_message;

// Add this line - it includes the generated gRPC code
pub mod log_service {
    tonic::include_proto!("logservice");
}

// Import the generated types
use log_service::{
    log_service_server::{LogService, LogServiceServer},
    LogRequest as ProtoLogRequest,  // Rename the imported type
    LogResponse
};

/// gRPC server for log messages
pub struct GrpcServer {
    config: ServerConfig,
    writer: Arc<LogWriter>,
}

//-----------------------------------------------------------------------------------------------

impl GrpcServer {
    /// Create new gRPC server
    pub fn new(config: &ServerConfig, writer: Arc<LogWriter>) -> Self {
        Self {
            config: config.clone(),
            writer,
        }
    }
    
    //-----------------------------------------------------------------------------------------------
    
    /// Run the gRPC server
    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let addr = format!("{}:{}", self.config.host, self.config.grpc_port).parse()?;
        let service = GrpcLogServiceImpl::new(&self.config, self.writer.clone());
        
        println!("{} : gRPC server listening on {}", self.config.name, addr);
        
        Server::builder()
            .add_service(LogServiceServer::new(service))
            .serve(addr)
            .await?;
            
        Ok(())
    }
}

//-----------------------------------------------------------------------------------------------

/// gRPC service implementation
pub struct GrpcLogServiceImpl {
    writer_tx: mpsc::Sender<String>,
    sequence_counter: Arc<AtomicU64>,
    name: String,
}

//-----------------------------------------------------------------------------------------------

impl GrpcLogServiceImpl {
    /// Create new gRPC service implementation
    pub fn new(config: &ServerConfig, writer: Arc<LogWriter>) -> Self {
        let writer_tx = writer.start_writer_task();
        let sequence_counter = Arc::new(AtomicU64::new(0));
        
        Self {
            writer_tx,
            sequence_counter,
            name: config.name.clone(),
        }
    }
}

//-----------------------------------------------------------------------------------------------

#[tonic::async_trait]
impl LogService for GrpcLogServiceImpl {
    /// Handle incoming gRPC log message
    async fn log_message(
        &self,
        request: Request<ProtoLogRequest>,  // Use the renamed type
    ) -> Result<Response<LogResponse>, Status> {
        let log_data = request.into_inner();
        
        // Convert to internal type and handle
        let internal_request = InternalLogRequest::from(log_data);  // Use the new name
        match handle_grpc_message(internal_request, self.writer_tx.clone(), self.sequence_counter.clone()).await {
            Ok(_) => {
                Ok(Response::new(LogResponse { success: true }))
            }
            Err(e) => {
                eprintln!("{} : failed to process gRPC message - {}", self.name, e);
                Err(Status::internal(format!("Failed to process log message: {}", e)))
            }
        }
    }
}

//-----------------------------------------------------------------------------------------------

/// Internal log request wrapper for internal use
pub struct InternalLogRequest {  // Renamed to avoid conflict
    pub timestamp: String,
    pub hostname: String,
    pub logger_name: String,
    pub level: i32,
    pub filename: String,
    pub function_name: String,
    pub line_number: String,
    pub message: String,
}

//-----------------------------------------------------------------------------------------------

// Conversion from protobuf to internal type
impl From<ProtoLogRequest> for InternalLogRequest {  // Use the renamed types
    fn from(request: ProtoLogRequest) -> Self {
        Self {
            timestamp: request.timestamp,
            hostname: request.hostname,
            logger_name: request.logger_name,
            level: request.level,
            filename: request.filename,
            function_name: request.function_name,
            line_number: request.line_number,
            message: request.message,
        }
    }
}