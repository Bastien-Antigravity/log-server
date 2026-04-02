//! TCP socket server for Cap'n Proto messages

use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

use crate::config::config::Config;
use crate::core::protocol_handlers::handle_tcp_message;
use crate::transport::safe_socket::SafeSocket;
use crate::utils::terminal_ui::print_internal_log;

/// TCP server for Cap'n Proto log messages
pub struct TcpServer {
    config: Config,
}

//-----------------------------------------------------------------------------------------------

impl TcpServer {
    /// Create new TCP server
    pub fn new(config: &Config) -> Self {
        Self {
            config: config.clone(),
        }
    }

    /// Get server name
    pub fn name(&self) -> &str {
        &self.config.name
    }

    //-----------------------------------------------------------------------------------------------

    /// Run the TCP server
    pub async fn run(
        &self,
        writer_tx: mpsc::Sender<String>,
        sequence_counter: Arc<AtomicU64>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let addr = format!("{}:{}", self.config.host, self.config.port);
        let listener = TcpListener::bind(&addr).await?;

        print_internal_log(
            "INFO",
            &self.config.name,
            "tcp_server.rs",
            "40",
            &format!("TCP server listening on {}", addr),
        );

        // Main server loop
        loop {
            let (socket, addr) = listener.accept().await?;
            let writer_tx = writer_tx.clone();
            let sequence_counter = sequence_counter.clone();
            let client_name = format!("{}_client_{}", self.config.name, addr);

            tokio::spawn(async move {
                if let Err(e) =
                    Self::handle_tcp_connection(socket, writer_tx, sequence_counter, &client_name)
                        .await
                {
                    print_internal_log(
                        "ERROR",
                        &client_name,
                        "tcp_server.rs",
                        "54",
                        &format!("{} - connection handler failed: {}", client_name, e),
                    );
                }
            });
        }
    }

    //-----------------------------------------------------------------------------------------------

    /// Handle individual TCP connection
    async fn handle_tcp_connection(
        socket: TcpStream,
        writer_tx: mpsc::Sender<String>,
        sequence_counter: Arc<AtomicU64>,
        name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut safe_socket = SafeSocket::new(socket);
        print_internal_log(
            "INFO",
            name,
            "tcp_server.rs",
            "71",
            &format!("client connected ({})", name),
        );

        loop {
            let bytes_read = safe_socket.receive_data().await?;

            if bytes_read.is_none() {
                print_internal_log(
                    "INFO",
                    name,
                    "tcp_server.rs",
                    "77",
                    &format!("client disconnected ({})", name),
                );
                break;
            }

            let data = bytes_read.unwrap().to_vec();

            // Connection closed, or corrupted message -> close connection, client socket have to manage reconnection
            if let Err(e) =
                handle_tcp_message(data, writer_tx.clone(), sequence_counter.clone(), name).await
            {
                print_internal_log(
                    "ERROR",
                    name,
                    "tcp_server.rs",
                    "85",
                    &format!("{} - message handling failed: {}", name, e),
                );
                break;
            }
        }

        Ok(())
    }
}
