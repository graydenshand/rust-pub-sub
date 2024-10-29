use rmp_serde::Serializer;
use rmpv::Value;
use serde::{Deserialize, Serialize};

use rmp_serde;
use std::error::Error;

use bytes::{Buf, BytesMut};

use crate::config;
use tokio;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
// use tokio::sync::mpsc::Receiver;
use tokio::sync::broadcast::{Sender, Receiver};
use tokio::time::Instant;
use log::info;
use std::future::Future;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Message {
    topic: String,
    value: Value,
    client_id: String,
}
impl Message {
    pub fn new(topic: &str, value: Value, client_id: &str) -> Message {
        Message {
            topic: String::from(topic),
            value,
            client_id: String::from(client_id)
        }
    }

    /// Get the topic
    pub fn topic(&self) -> &str {
        &self.topic
    }

    /// Get the value
    pub fn value(&self) -> &Value {
        &self.value
    }

    /// Get the client_id
    pub fn client_id(&self) -> &str {
        &self.client_id
    }
}

/// Read half of a connection
pub struct MessageReader {
    stream: OwnedReadHalf,
    buffer: BytesMut,
}

impl MessageReader {
    pub fn new(stream: OwnedReadHalf) -> MessageReader {
        MessageReader {
            stream,
            buffer: BytesMut::with_capacity(4096),
        }
    }

    /// Read a Message from buffer
    ///
    /// Returns a tuple containing the Message and the number of bytes read
    fn parse_value(&mut self) -> (Option<Message>, usize) {
        let buf = &mut &self.buffer[..];
        let start_len = buf.len();
        let message = rmp_serde::decode::from_read::<&mut &[u8], Message>(buf).ok();
        let end_len = buf.len();
        (message, (start_len - end_len))
    }

    /// Read a message from the stream
    pub async fn read_value(&mut self) -> Result<Option<Message>, Box<dyn Error>> {
        loop {
            // Attempt to parse a frame from the buffered data. If
            // enough data has been buffered, the frame is
            // returned.
            let (message, bytes_read) = self.parse_value();
            if let Some(m) = message {
                self.buffer.advance(bytes_read);
                return Ok(Some(m));
            }

            // There is not enough buffered data to read a frame.
            // Attempt to read more data from the socket.
            //
            // On success, the number of bytes is returned. `0`
            // indicates "end of stream".
            self.stream.readable().await?;
            let bytes_read = self.stream.read_buf(&mut self.buffer).await?;
            if bytes_read == 0 {
                // The remote closed the connection. For this to be
                // a clean shutdown, there should be no data in the
                // read buffer. If there is, this means that the
                // peer closed the socket while sending a frame.
                if self.buffer.is_empty() {
                    return Ok(None);
                } else {
                    return Err("connection reset by peer".into());
                }
            }
        }
    }

    /// Bind a function to this stream, invoke for every message received.
    /// 
    /// Returns the return value of the last function invocation
    pub async fn bind<F>(
        &mut self,
        mut handler: F,
    ) -> Result<(), Box<dyn Error>> where F: FnMut(Message) {
        loop {
            let message = self.read_value().await.ok();
            if message.is_none() || message.as_ref().unwrap().is_none() {
                // Terminate loop
                return Ok(());
            } else {
                let m: Message = message
                    .expect("message is Ok")
                    .expect("message is not None");
                
                handler(m);
            }
        }
    }
}

/// Write half of a connection
pub struct MessageWriter {
    stream: OwnedWriteHalf,
}

impl MessageWriter {
    pub fn new(stream: OwnedWriteHalf) -> MessageWriter {
        MessageWriter { stream }
    }

    /// Send a message over this connection
    pub async fn send(&mut self, message: Message) -> Result<(), Box<dyn Error>> {
        let mut buf = Vec::new();
        message.serialize(&mut Serializer::new(&mut buf)).unwrap();
        self.stream.write(&mut buf).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_serde() {}
}
