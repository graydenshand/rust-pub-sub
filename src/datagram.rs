use rmp_serde::Serializer;
use rmpv::Value;
use serde::{Deserialize, Serialize};


use rmp_serde;
use std::error::Error;

use std::time::Instant;
use tokio;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use bytes::{Buf, BytesMut};

pub const SYSTEM_TOPIC_PREFIX: &'static str = "!system";
pub const SUBSCRIBE_TOPIC: &'static str = "/subscribe";

#[derive(Serialize, Deserialize, Debug)]
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

pub struct MessageReader {
    reader: OwnedReadHalf,
    buffer: BytesMut,
}

impl MessageReader {
    pub fn new(
        stream: OwnedReadHalf,
    ) -> MessageReader {
        MessageReader {
            reader: stream,
            buffer: BytesMut::with_capacity(4096),
        }
    }

    pub async fn receive_loop(&mut self) -> Result<(), Box<dyn Error>> {
        let start = Instant::now();

        let mut count = 0;

        loop {
            let message = self.read_value().await;
            if message.is_err() || message.as_ref().unwrap().is_none() {
                let end = Instant::now();
                let seconds = (end - start).as_millis() as f64 / 1000.0;
                println!(
                    "{} messages received in {}s - {} m/s",
                    count,
                    seconds,
                    (count as f64 / seconds).round()
                );
                println!("Disconnected");
                return Ok(());
            } else {
                // Do something with the message
                // println!("{:?}", message?.unwrap());
                let _m: Message = message?.expect("Already checked this is not none");
                
                // TODO: Invoke callback 
            }

            count += 1;
        }
    }

    fn parse_value(&mut self) -> (Option<Message>, usize) {
        let buf = &mut &self.buffer[..];
        let start_len = buf.len();
        let message = rmp_serde::decode::from_read::<&mut &[u8], Message>(buf).ok();
        let end_len = buf.len();

        (message, (start_len - end_len))
    }

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
            self.reader.readable().await?;
            let bytes_read = self.reader.read_buf(&mut self.buffer).await?;
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

pub struct MessageWriter {
    writer: OwnedWriteHalf,
}

impl MessageWriter {
    pub fn new(stream: OwnedWriteHalf) -> MessageWriter {
        MessageWriter { writer: stream }
    }

    pub async fn send(&mut self, message: Message) -> Result<(), Box<dyn Error>> {
        let mut buf = Vec::new();
        message.serialize(&mut Serializer::new(&mut buf)).unwrap();
        self.writer.write(&mut buf).await?;
        Ok(())
    }

    pub async fn write_loop(&mut self, ) {
        loop {
            // TODO replace with channel listener
            self.send(Message::new("test", Value::Boolean(true))).await.unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    

    #[test]
    fn test_serde() {}
}
