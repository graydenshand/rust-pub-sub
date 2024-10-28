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

    // Listen for messages over connection and forward over this connection
    pub async fn subscribe_to_channel(
        &mut self,
        tx: Sender<Message>,
    ) -> Result<(), Box<dyn Error>> {
        let start = Instant::now();

        let mut count = 0;

        loop {
            let message = self.read_value().await.ok();
            if message.is_none() || message.as_ref().unwrap().is_none() {
                // Log stats about messages received from client
                let end = Instant::now();
                let seconds = (end - start).as_millis() as f64 / 1000.0;
                info!(
                    "DISCONNECT - {count} messages received in {seconds}s - {} m/s",
                    (count as f64 / seconds).round()
                );
                // Terminate loop
                return Ok(());
            } else {
                let m: Message = message
                    .expect("message is Ok")
                    .expect("message is not None");
                // broadcast message
                tx.send(m).expect("Message is sent");
            }

            count += 1;
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
