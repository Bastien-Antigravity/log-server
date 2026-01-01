//! TCP socket server for Cap'n Proto messages

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use crate::common::config::ServerConfig;
use crate::common::safe_socket::SafeSocket;
use crate::core::writers::LogWriter;
use crate::core::handlers::handle_tcp_message;




/// TCP server for Cap'n Proto log messages
pub struct TcpServer {
    config: ServerConfig,
    writer: Arc<LogWriter>,
}

//-----------------------------------------------------------------------------------------------

impl TcpServer {
    /// Create new TCP server
    pub fn new(config: &ServerConfig, writer: Arc<LogWriter>) -> Self {
        Self {
            config: config.clone(),
            writer,
        }
    }
    
    //-----------------------------------------------------------------------------------------------
    
    /// Run the TCP server
    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let addr = format!("{}:{}", self.config.host, self.config.port);
        let listener = TcpListener::bind(&addr).await?;
        
        println!("{} : TCP server listenning on {}", self.config.name, addr);
        
        let sequence_counter = Arc::new(AtomicU64::new(0));
        let writer_tx = self.writer.start_writer_task();
        
        // Main server loop
        loop {
            let (socket, addr) = listener.accept().await?;
            let writer_tx = writer_tx.clone();
            let sequence_counter = sequence_counter.clone();
            let client_name = format!("{}_client_{}", self.config.name, addr);
            
            tokio::spawn(async move {
                if let Err(e) = Self::handle_tcp_connection(socket, writer_tx, sequence_counter, &client_name).await {
                    eprintln!("{} : connection handler failed - {}", client_name, e);
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
        println!("{} : client connected", name);

        loop {
            let bytes_read = safe_socket.receive_data().await?;

            if bytes_read.is_none() {
                println!("{} : client disconnected", name);
                break;
            }

            let data = bytes_read.unwrap().to_vec();
            
            // Connection closed, or corrupted message -> close connection, client socket have to manage reconnection
            if let Err(e) = handle_tcp_message(data, writer_tx.clone(), sequence_counter.clone(), name).await {
                eprintln!("{} : message handling failed - {}", name, e);
                break;
            }
        }
        
        Ok(())
    }
}