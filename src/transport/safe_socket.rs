//! ESSENTIAL PROCESS:
//! Safe TCP socket wrapper with 4-byte big-endian length prefix framing,
//! zero-length heartbeat packet filtering, and split read/write halves.
//!
//! DATA FLOW:
//! 1. TcpStream -> SafeSocket::new() -> (SafeSocketReader, SafeSocketWriter)
//! 2. SafeSocketReader::receive() -> Discards heartbeats -> Returns payload bytes
//!
//! KEY PARAMETERS:
//! - MAX_MESSAGE_SIZE: 10MB guard against unbounded allocation attacks.
//! - 4-byte big-endian integer length framing header.
// cli:allow-print

use bytes::BytesMut;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;

/// Reader half of a safe socket
pub struct SafeSocketReader {
    reader: OwnedReadHalf,
}

/// Maximum allowed message size (10MB) to prevent OOM attacks.
const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024;

/// Writer half of a safe socket
pub struct SafeSocketWriter {
    writer: OwnedWriteHalf,
}

/// Safe TCP socket with message framing
pub struct SafeSocket {
    pub reader: SafeSocketReader,
    pub writer: SafeSocketWriter,
}

// -----------------------------------------------------------------------------

impl SafeSocketReader {
    /// Receive framed data from socket. Returns empty BytesMut on 0-length heartbeat frames.
    pub async fn receive_data(&mut self) -> io::Result<Option<BytesMut>> {
        // big-endian u32 length prefix
        let mut length_buf = [0u8; 4];
        let n = self.reader.read_exact(&mut length_buf).await;

        if let Err(e) = n {
            if e.kind() == io::ErrorKind::UnexpectedEof {
                return Ok(None);
            }
            return Err(e);
        }

        let slen = u32::from_be_bytes(length_buf) as usize;

        // SECURITY: Prevent OOM by enforcing a maximum message size
        if slen > MAX_MESSAGE_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "message too large: {} bytes (max: {})",
                    slen, MAX_MESSAGE_SIZE
                ),
            ));
        }

        // HEARTBEAT: If length is 0, return an empty BytesMut to let the caller handle it (e.g. reset timeouts)
        if slen == 0 {
            return Ok(Some(BytesMut::new()));
        }

        // Read exactly slen bytes directly into preallocated buffer without intermediate stack copying
        let mut chunk = BytesMut::zeroed(slen);
        if let Err(e) = self.reader.read_exact(&mut chunk).await {
            if e.kind() == io::ErrorKind::UnexpectedEof {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    format!("connection closed mid-message (expected {} bytes)", slen),
                ));
            }
            return Err(e);
        }
        Ok(Some(chunk))
    }
}

// -----------------------------------------------------------------------------

impl SafeSocketWriter {
    /// Send framed data to socket
    pub async fn send_data(&mut self, data: &[u8]) -> io::Result<()> {
        let len = data.len() as u64;
        if len > MAX_MESSAGE_SIZE as u64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "outgoing message size {len} exceeds MAX_MESSAGE_SIZE ({MAX_MESSAGE_SIZE})"
                ),
            ));
        }
        let len_u32 = len as u32;
        self.writer.write_all(&len_u32.to_be_bytes()).await?;
        self.writer.write_all(data).await?;
        self.writer.flush().await
    }

    /// Send a 0-length heartbeat frame
    pub async fn send_heartbeat(&mut self) -> io::Result<()> {
        self.writer.write_all(&0u32.to_be_bytes()).await?;
        self.writer.flush().await
    }

    /// Shutdown the underlying connection
    pub async fn shutdown(&mut self) -> io::Result<()> {
        self.writer.shutdown().await
    }
}

// -----------------------------------------------------------------------------

impl SafeSocket {
    /// Create new safe socket by splitting a TcpStream
    pub fn new(conn: TcpStream) -> Self {
        let (reader, writer) = conn.into_split();
        SafeSocket {
            reader: SafeSocketReader { reader },
            writer: SafeSocketWriter { writer },
        }
    }

    /// Split into its component halves
    pub fn split(self) -> (SafeSocketReader, SafeSocketWriter) {
        (self.reader, self.writer)
    }
}

// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn test_heartbeat_skip() {
        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(l) => l,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                eprintln!(
                    "Skipping test_heartbeat_skip: bind PermissionDenied in restricted sandbox"
                );
                return;
            }
            Err(e) => panic!("bind failed: {e}"),
        };
        let addr = listener.local_addr().unwrap();

        let server_task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();

            // Send Heartbeat (4 bytes of 0)
            socket.write_all(&0u32.to_be_bytes()).await.unwrap();

            // Send Data Frame (4 bytes len + "HELLO")
            let msg = b"HELLO";
            socket
                .write_all(&(msg.len() as u32).to_be_bytes())
                .await
                .unwrap();
            socket.write_all(msg).await.unwrap();
        });

        let client_stream = match TcpStream::connect(addr).await {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                eprintln!(
                    "Skipping test_heartbeat_skip: connect PermissionDenied in restricted sandbox"
                );
                return;
            }
            Err(e) => panic!("connect failed: {e}"),
        };
        let safe_socket = SafeSocket::new(client_stream);
        let (mut reader, _) = safe_socket.split();

        // receive_data should return an empty BytesMut for the heartbeat
        let mut data = reader.receive_data().await.unwrap().unwrap();
        while data.is_empty() {
            data = reader.receive_data().await.unwrap().unwrap();
        }
        assert_eq!(data.as_ref(), b"HELLO");

        server_task.await.unwrap();
    }

    // -----------------------------------------------------------------------------

    #[tokio::test]
    async fn test_oversized_message_rejected() {
        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(l) => l,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(e) => panic!("bind failed: {e}"),
        };
        let addr = listener.local_addr().unwrap();

        let server_task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let oversized = (MAX_MESSAGE_SIZE + 1) as u32;
            socket.write_all(&oversized.to_be_bytes()).await.unwrap();
        });

        let client_stream = match TcpStream::connect(addr).await {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(e) => panic!("connect failed: {e}"),
        };
        let safe_socket = SafeSocket::new(client_stream);
        let (mut reader, _) = safe_socket.split();

        let res = reader.receive_data().await;
        assert!(res.is_err(), "Oversized message must be rejected");
        assert_eq!(res.unwrap_err().kind(), std::io::ErrorKind::InvalidData);

        let _ = server_task.await;
    }

    // -----------------------------------------------------------------------------

    #[tokio::test]
    async fn test_unexpected_eof_mid_message() {
        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(l) => l,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(e) => panic!("bind failed: {e}"),
        };
        let addr = listener.local_addr().unwrap();

        let server_task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            // Send length 20, but only 5 bytes, then drop socket immediately
            let len = 20u32;
            socket.write_all(&len.to_be_bytes()).await.unwrap();
            socket.write_all(b"12345").await.unwrap();
            drop(socket);
        });

        let client_stream = match TcpStream::connect(addr).await {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(e) => panic!("connect failed: {e}"),
        };
        let safe_socket = SafeSocket::new(client_stream);
        let (mut reader, _) = safe_socket.split();

        let res = reader.receive_data().await;
        assert!(
            res.is_err(),
            "Mid-message connection drop must return error"
        );
        assert_eq!(res.unwrap_err().kind(), std::io::ErrorKind::UnexpectedEof);

        let _ = server_task.await;
    }

    // -----------------------------------------------------------------------------

    #[tokio::test]
    async fn test_outgoing_oversized_message_rejected() {
        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(l) => l,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(e) => panic!("bind failed: {e}"),
        };
        let addr = listener.local_addr().unwrap();

        let server_task = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let _ = socket;
        });

        let client_stream = match TcpStream::connect(addr).await {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(e) => panic!("connect failed: {e}"),
        };
        let safe_socket = SafeSocket::new(client_stream);
        let (_, mut writer) = safe_socket.split();

        let oversized_len = MAX_MESSAGE_SIZE + 1;
        let dummy = [0u8; 8];
        let huge_ref: &[u8] = unsafe { std::slice::from_raw_parts(dummy.as_ptr(), oversized_len) };

        let res = writer.send_data(huge_ref).await;
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().kind(), std::io::ErrorKind::InvalidInput);

        let _ = server_task.await;
    }
}
