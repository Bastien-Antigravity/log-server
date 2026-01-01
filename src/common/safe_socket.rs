//! Safe TCP socket wrapper with message framing

use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use bytes::{BytesMut, BufMut};





/// Safe TCP socket with message framing
pub struct SafeSocket {
    conn: TcpStream,
}

//-----------------------------------------------------------------------------------------------

impl SafeSocket {
    /// Create new safe socket
    pub fn new(conn: TcpStream) -> Self {
        SafeSocket { conn }
    }
    
    //-----------------------------------------------------------------------------------------------
    
    /// Receive framed data from socket
    pub async fn receive_data(&mut self) -> io::Result<Option<BytesMut>> {
        // big-endian u32 length prefix
        let mut length_buf = [0u8; 4];
        let n = self.conn.read(&mut length_buf).await?; 
        if n < 4 {
            return Ok(None);
        }
        let slen = u32::from_be_bytes(length_buf) as usize;
        let mut chunk = BytesMut::with_capacity(slen);
        
        while chunk.len() < slen {
            let remaining = slen - chunk.len();
            let mut buf = vec![0u8; remaining];
            let n = self.conn.read(&mut buf).await?; 
            if n == 0 {
                return Ok(None);
            }
            chunk.put_slice(&buf[..n]);
        }
        Ok(Some(chunk))
    }
}

//-----------------------------------------------------------------------------------------------

impl Drop for SafeSocket {
    fn drop(&mut self) {
        let _ = self.conn.shutdown();
    }
}