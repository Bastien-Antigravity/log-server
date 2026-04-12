//! Safe TCP socket wrapper with message framing and heartbeat support

use bytes::{BufMut, BytesMut};
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;

/// Reader half of a safe socket
pub struct SafeSocketReader {
    reader: OwnedReadHalf,
}

/// Writer half of a safe socket
pub struct SafeSocketWriter {
    writer: OwnedWriteHalf,
}

/// Safe TCP socket with message framing
pub struct SafeSocket {
    pub reader: SafeSocketReader,
    pub writer: SafeSocketWriter,
}

//-----------------------------------------------------------------------------------------------

impl SafeSocketReader {
    /// Receive framed data from socket. Automatically skips 0-length heartbeat frames.
    pub async fn receive_data(&mut self) -> io::Result<Option<BytesMut>> {
        loop {
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
            
            // HEARTBEAT: If length is 0, skip and wait for next frame
            if slen == 0 {
                continue;
            }

            let mut chunk = BytesMut::with_capacity(slen);
            
            // Read exactly slen bytes
            while chunk.len() < slen {
                let remaining = slen - chunk.len();
                let mut buf = vec![0u8; remaining];
                let n = self.reader.read(&mut buf).await?;
                if n == 0 {
                    return Ok(None);
                }
                chunk.put_slice(&buf[..n]);
            }
            return Ok(Some(chunk));
        }
    }
}

//-----------------------------------------------------------------------------------------------

impl SafeSocketWriter {
    /// Send framed data to socket
    pub async fn send_data(&mut self, data: &[u8]) -> io::Result<()> {
        let len = data.len() as u32;
        self.writer.write_all(&len.to_be_bytes()).await?;
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

//-----------------------------------------------------------------------------------------------

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

//-----------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn test_heartbeat_skip() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server_task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            
            // Send Heartbeat (4 bytes of 0)
            socket.write_all(&0u32.to_be_bytes()).await.unwrap();
            
            // Send Data Frame (4 bytes len + "HELLO")
            let msg = b"HELLO";
            socket.write_all(&(msg.len() as u32).to_be_bytes()).await.unwrap();
            socket.write_all(msg).await.unwrap();
        });

        let client_stream = TcpStream::connect(addr).await.unwrap();
        let safe_socket = SafeSocket::new(client_stream);
        let (mut reader, _) = safe_socket.split();

        // receive_data should skip the heartbeat and return "HELLO"
        let data = reader.receive_data().await.unwrap().unwrap();
        assert_eq!(data.as_ref(), b"HELLO");

        server_task.await.unwrap();
    }
}
