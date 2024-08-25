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
use tokio::sync::mpsc::Receiver;
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    topic: String,
    value: Value,
}
impl Message {
    pub fn new(topic: &str, value: Value) -> Message {
        Message {
            topic: String::from(topic),
            value,
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

    /// Parse a MessagePack Value from buffer
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

    // Listen for messages over async channel and forward over this connection
    pub async fn subscribe_to_channel(
        &mut self,
        mut rx: Receiver<Message>,
    ) -> Result<(), Box<dyn Error>> {
        while let Some(message) = rx.recv().await {
            self.send(message).await?
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_serde() {}
}
