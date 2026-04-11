//! Main log server orchestrator
//!
//! Coordinates TCP and gRPC servers with shared file writer.

use std::sync::Arc;

use crate::config::config::Config;
use crate::core::log_writer::LogWriter;
use crate::servers::grpc_server::GrpcServer;
use crate::servers::tcp_server::TcpServer;
use crate::utils::terminal_ui::print_internal_log;

/// Main log server orchestrator
pub struct LogServer {
    name: String,
    config: Config,
    writer: Arc<LogWriter>,
    enable_grpc: bool,
}

//-----------------------------------------------------------------------------------------------

impl LogServer {
    /// Create new log server instance
    pub async fn new(
        name: &str,
        host: &str,
        port: u16,
        grpc_port: u16,
        enable_grpc: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let config = Config::new(name, host, port, grpc_port);

        // Create log directory
        crate::utils::helpers::create_log_folder("logs")?;

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
        print_internal_log(
            "INFO",
            &self.name,
            "log_server.rs",
            "run",
            "52",
            &format!("{} : starting server components. .  .", self.name),
        );

        // Start a single writer task for all components to share
        let writer_tx = self.writer.start_writer_task();

        // Centralized sequence counter for global ordering
        let sequence_counter = Arc::new(std::sync::atomic::AtomicU64::new(0));

        // Initialize the internal logger so it can also write to files
        crate::utils::terminal_ui::set_internal_logger(writer_tx.clone(), sequence_counter.clone());

        print_internal_log(
            "INFO",
            &self.name,
            "log_server.rs",
            "run",
            "70",
            &format!("{} : internal logger initialized - writer(s) ready !", self.name),
        );

        // Start TCP server (always)
        let tcp_server = TcpServer::new(&self.config);
        let tcp_writer_tx = writer_tx.clone();
        let tcp_sequence_counter = sequence_counter.clone();

        let tcp_handle = tokio::spawn(async move {
            if let Err(e) = tcp_server.run(tcp_writer_tx, tcp_sequence_counter).await {
                print_internal_log(
                    "ERROR",
                    tcp_server.name(),
                    "log_server.rs",
                    "run",
                    "86",
                    &format!("{} : TCP server error: {e}", tcp_server.name()),
                );
            }
        });

        // Conditionally start gRPC server
        let grpc_handle = if self.enable_grpc {
            let grpc_server = GrpcServer::new(&self.config);
            let grpc_writer_tx = writer_tx.clone();
            let grpc_sequence_counter = sequence_counter.clone();

            Some(tokio::spawn(async move {
                if let Err(e) = grpc_server.run(grpc_writer_tx, grpc_sequence_counter).await {
                    print_internal_log(
                        "ERROR",
                        grpc_server.name(),
                        "log_server.rs",
                        "run",
                        "105",
                        &format!("gRPC server error: {e}"),
                    );
                }
            }))
        } else {
            None
        };

        print_internal_log(
            "INFO",
            &self.name,
            "log_server.rs",
            "run",
            "119",
            &format!("{} : all server components started !", self.name),
        );

        // Wait for servers to complete
        let _ = tcp_handle.await;

        if let Some(grpc_handle) = grpc_handle {
            let _ = grpc_handle.await;
        }

        Ok(())
    }
}
