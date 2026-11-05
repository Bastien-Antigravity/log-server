//! Main log server orchestrator
//!
//! Coordinates TCP and gRPC servers with shared file writer.

use std::sync::Arc;

use crate::config::config::Config;
use crate::facade::log_writer::LogWriter;
use crate::line_str;
use crate::servers::grpc_server::LogBridgeGateway;
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
            line_str!(),
            &format!("starting server components. .  ."),
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
            line_str!(),
            &format!("internal logger initialized - writer(s) ready !"),
        );

        // Start TCP server (always)
        let tcp_server = TcpServer::new(&self.config);
        let tcp_writer_tx = writer_tx.clone();
        let tcp_sequence_counter = sequence_counter.clone();
        let identity = self.name.clone();

        let tcp_handle = tokio::spawn(async move {
            if let Err(e) = tcp_server.run(tcp_writer_tx, tcp_sequence_counter).await {
                print_internal_log(
                    "ERROR",
                    &identity,
                    "log_server.rs",
                    "run",
                    line_str!(),
                    &format!("TCP server error: {e}"),
                );
                Err(e.to_string())
            } else {
                Ok(())
            }
        });

        // Conditionally start gRPC bridge gateway
        let identity_grpc = self.name.clone();
        let grpc_handle = if self.enable_grpc {
            let grpc_server = LogBridgeGateway::new(&self.config);
            let grpc_writer_tx = writer_tx.clone();
            let grpc_sequence_counter = sequence_counter.clone();

            Some(tokio::spawn(async move {
                if let Err(e) = grpc_server.run(grpc_writer_tx, grpc_sequence_counter).await {
                    print_internal_log(
                        "ERROR",
                        &identity_grpc,
                        "log_server.rs",
                        "run",
                        line_str!(),
                        &format!("Log Bridge error: {e}"),
                    );
                    Err(e.to_string())
                } else {
                    Ok(())
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
            line_str!(),
            &format!("all server components started !"),
        );

        // Wait for servers to complete; abort if either fails
        tokio::select! {
            res = tcp_handle => {
                match res {
                    Ok(Ok(())) => Ok(()),
                    Ok(Err(e)) => Err(e.into()),
                    Err(join_err) => Err(Box::new(join_err) as Box<dyn std::error::Error>),
                }
            }
            res = async {
                if let Some(h) = grpc_handle {
                    h.await
                } else {
                    std::future::pending().await
                }
            } => {
                match res {
                    Ok(Ok(())) => Ok(()),
                    Ok(Err(e)) => Err(e.into()),
                    Err(join_err) => Err(Box::new(join_err) as Box<dyn std::error::Error>),
                }
            }
        }
    }
}
