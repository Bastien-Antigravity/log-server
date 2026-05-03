//! TCP socket server for Cap'n Proto messages

use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};
use crate::models::log_packet::LogPacket;

use crate::config::config::Config;
use crate::core::protocol_handlers::{handle_tcp_message, identify_client_from_handshake};
use crate::line_str;
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
        writer_tx: mpsc::Sender<LogPacket>,
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
            "run",
            line_str!(),
            &format!("{} : TCP server listening on {}", self.config.name, addr),
        );

        // Main server loop
        loop {
            let (socket, addr) = listener.accept().await?;
            let writer_tx = writer_tx.clone();
            let sequence_counter = sequence_counter.clone();
            let client_name = format!("{name}_client_{addr}", name = self.config.name, addr = addr);
            let server_name = self.config.name.clone();
            tokio::spawn(async move {
                if let Err(e) =
                    Self::handle_tcp_connection(socket, writer_tx, sequence_counter, &server_name)
                        .await
                {
                    print_internal_log(
                        "ERROR",
                        &client_name,
                        "tcp_server.rs",
                        "run",
                        line_str!(),
                        &format!("{client_name} : connection handler failed: {e}"),
                    );
                }
            });
        }
    }

    //-----------------------------------------------------------------------------------------------

    /// Handle individual TCP connection
    async fn handle_tcp_connection(
        socket: TcpStream,
        writer_tx: mpsc::Sender<LogPacket>,
        sequence_counter: Arc<AtomicU64>,
        name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let peer_addr = socket.peer_addr()?;
        let local_addr = socket.local_addr()?;
        let peer_ip = peer_addr.ip();
        let peer_port = peer_addr.port();
        let local_ip = local_addr.ip();
        let local_port = local_addr.port();

        let safe_socket = SafeSocket::new(socket);
        let (mut reader, mut writer) = safe_socket.split();

        print_internal_log(
            "INFO",
            name,
            "tcp_server.rs",
            "handle_tcp_connection",
            line_str!(),
            &format!("{name} : TCP connection established from '{peer_ip}' port '{peer_port}' to host '{local_ip}' port '{local_port}'"),
        );

        // 1. Initial Handshake / Identity Detection (Mandatory - FEAT-006)
        // SECURITY: 5-second timeout to prevent connection-holding (Slow-Loris) attacks.
        let first_bytes = match timeout(Duration::from_secs(5), reader.receive_data()).await {
            Ok(res) => res?,
            Err(_) => return Err(format!("{name} : handshake timeout from {peer_ip}. Closing connection.").into()),
        };
        let actual_client_name;

        if let Some(data) = first_bytes {
            // Unpacked Cap'n Proto messages (like HelloMsg) start with segment count 0 (4 zero bytes)
            if data.len() >= 4 && &data[0..4] == &[0, 0, 0, 0] {
                if let Ok(identity) = identify_client_from_handshake(&data) {
                    actual_client_name = format!("{name}_{identity}");
                    print_internal_log(
                        "INFO",
                        name,
                        "tcp_server.rs",
                        "handle_tcp_connection",
                        line_str!(),
                        &format!("{name} : client identified via handshake as '{identity}'"),
                    );
                } else {
                    return Err(format!("{name} : malformed handshake received from {peer_ip}. Closing connection.").into());
                }
            } else {
                return Err(format!("{name} : mandatory handshake skipped by {peer_ip}. Closing connection.").into());
            }
        } else {
            return Ok(()); // Connection closed before handshake
        }

        // Spawn background heartbeat task (every 10 seconds)
        let heartbeat_client_name = actual_client_name.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                if let Err(e) = writer.send_heartbeat().await {
                    // Only log if it's not a closed connection error
                    if e.kind() != tokio::io::ErrorKind::BrokenPipe
                        && e.kind() != tokio::io::ErrorKind::ConnectionAborted
                    {
                        print_internal_log(
                            "DEBUG",
                            &heartbeat_client_name,
                            "tcp_server.rs",
                            "heartbeat_task",
                            line_str!(),
                            &format!("Heartbeat failed for {heartbeat_client_name}: {e}"),
                        );
                    }
                    break;
                }
            }
        });

        // 2. Main Message Loop

        // 3. Main Message Loop
        loop {
            let bytes_read = reader.receive_data().await?;

            if bytes_read.is_none() {
                print_internal_log(
                    "INFO",
                    &actual_client_name,
                    "tcp_server.rs",
                    "handle_tcp_connection",
                    line_str!(),
                    &format!("{actual_client_name} : TCP connection has been closed"),
                );
                break;
            }

            let data = bytes_read.unwrap().to_vec();

            if let Err(e) =
                handle_tcp_message(data, writer_tx.clone(), sequence_counter.clone(), &actual_client_name).await
            {
                print_internal_log(
                    "ERROR",
                    &actual_client_name,
                    "tcp_server.rs",
                    "handle_tcp_connection",
                    line_str!(),
                    &format!("{actual_client_name} : message handling failed: {e}"),
                );
                break;
            }
        }

        Ok(())
    }
}
