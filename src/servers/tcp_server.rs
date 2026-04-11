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
        let addr = format!(
            "{host}:{port}",
            host = self.config.host,
            port = self.config.port
        );
        let listener = TcpListener::bind(&addr).await?;

        print_internal_log(
            "INFO",
            &self.config.name,
            "tcp_server.rs",
            "52",
            &format!("TCP server listening on {addr}"),
        );

        // Main server loop
        loop {
            let (socket, addr) = listener.accept().await?;
            let writer_tx = writer_tx.clone();
            let sequence_counter = sequence_counter.clone();
            let client_name = format!("{name}_client_{addr}", name = self.config.name, addr = addr);

            tokio::spawn(async move {
                if let Err(e) =
                    Self::handle_tcp_connection(socket, writer_tx, sequence_counter, &client_name)
                        .await
                {
                    print_internal_log(
                        "ERROR",
                        &client_name,
                        "tcp_server.rs",
                        "72",
                        &format!("{client_name} - connection handler failed: {e}"),
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
        let peer_addr = socket.peer_addr()?;
        let local_addr = socket.local_addr()?;
        let peer_ip = peer_addr.ip();
        let peer_port = peer_addr.port();
        let local_ip = local_addr.ip();
        let local_port = local_addr.port();

        let mut safe_socket = SafeSocket::new(socket);
        print_internal_log(
            "INFO",
            name,
            "tcp_server.rs",
            "101",
            &format!("{name} : TCP connection established from '{peer_ip}' port '{peer_port}' to host '{local_ip}' port '{local_port}'"),
        );

        loop {
            let bytes_read = safe_socket.receive_data().await?;

            if bytes_read.is_none() {
                print_internal_log(
                    "INFO",
                    name,
                    "tcp_server.rs",
                    "113",
                    &format!("{name} : TCP connection has been closed from '{peer_ip}' port '{peer_port}' to host '{local_ip}' port '{local_port}'"),
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
                    "129",
                    &format!("{name} : message handling failed: {e}"),
                );
                break;
            }
        }

        Ok(())
    }
}
