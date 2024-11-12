use rmp_serde::Serializer;
use rmpv::Value;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use rmp_serde;
use std::error::Error;

use bytes::{Buf, BytesMut};

use std::fmt;
use std::future::Future;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Message {
    pub topic: String,
    pub value: Value,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum Command {
    /// Publish a message to a topic
    Publish { message: Message },
    /// Subscribe to a topic
    Subscribe { pattern: String },
}
impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Publish { message } => {
                write!(f, "PUBLISH - {} - {:?}", message.topic, message.value)
            }
            Self::Subscribe { pattern } => {
                write!(f, "SUBSCRIBE - {pattern}")
            }
        }
    }
}
/// Read a msg_pack value from a stream
pub async fn read_stream<T, S>(
    stream: &mut T,
    mut buffer: &mut BytesMut,
) -> Result<Option<S>, Box<dyn Error>>
where
    T: AsyncReadExt + std::marker::Unpin,
    S: DeserializeOwned,
{
    loop {
        // Attempt to parse a frame from the buffered data. If
        // enough data has been buffered, the frame is
        // returned.
        let buf = &mut &buffer[..];
        let start_len = buf.len();
        let command = rmp_serde::decode::from_read::<&mut &[u8], S>(buf).ok();
        let bytes_read = start_len - buf.len();

        if let Some(c) = command {
            buffer.advance(bytes_read);
            return Ok(Some(c));
        }

        // There is not enough buffered data to read a frame.
        // Attempt to read more data from the socket.
        //
        // On success, the number of bytes is returned. `0`
        // indicates "end of stream".
        // stream.readable().await?;
        let bytes_read = stream.read_buf(&mut buffer).await?;
        if bytes_read == 0 {
            // The remote closed the connection. For this to be
            // a clean shutdown, there should be no data in the
            // read buffer. If there is, this means that the
            // peer closed the socket while sending a frame.
            if buffer.is_empty() {
                return Ok(None);
            } else {
                return Err("connection reset by peer".into());
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Datagram {
    /// The command to execute
    pub command: Command,
    /// The sender of the command
    pub sender: String,
}
impl Datagram {
    pub fn new(command: Command, sender: &str) -> Datagram {
        Datagram {
            command,
            sender: sender.to_string(),
        }
    }
}

/// Send a rust msgpack value over a stream
pub async fn send_rmp_value<T, D>(stream: &mut T, value: D) -> Result<(), Box<dyn Error>>
where
    T: AsyncWrite + std::marker::Unpin,
    D: Serialize,
{
    let mut buf = Vec::new();
    value.serialize(&mut Serializer::new(&mut buf)).unwrap();
    stream.write(&mut buf).await?;
    Ok(())
}

/// Bind an async function to this stream, invoke for every value received.
pub async fn bind_stream<T, F, Fut, S>(mut stream: T, mut handler: F) -> Result<(), Box<dyn Error>>
where
    T: AsyncRead + std::marker::Unpin,
    F: FnMut(S) -> Fut,
    Fut: Future<Output = ()>,
    S: DeserializeOwned,
{
    let mut buf = BytesMut::with_capacity(4096);
    loop {
        let value = read_stream(&mut stream, &mut buf).await?;
        if let Some(v) = value {
            // Invoke closure
            handler(v).await;
        } else {
            // End of stream
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::io::Cursor;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn send_and_receive() {
        // Construct a command
        let topic = "test".into();
        let value = Value::from("test");
        let message = Message { topic, value };
        let command = Command::Publish { message };
        let d1 = Datagram::new(command, "sender");

        // Send the command
        let mut stream = Vec::new();
        send_rmp_value(&mut stream, d1.clone()).await.unwrap();

        // Receive the command
        let mut buffer = BytesMut::new();
        let mut cursor = Cursor::new(stream);
        let d2 = read_stream(&mut cursor, &mut buffer)
            .await
            .unwrap()
            .unwrap();

        // Equality check
        assert_eq!(d1, d2);
    }

    #[tokio::test]
    async fn test_bind_stream() {
        // Construct a command
        let topic = "test".into();
        let value = Value::from("test");
        let sent_command = Command::Publish {
            message: Message { topic, value },
        };
        let datagram = Datagram::new(sent_command, "source");

        // Send the command
        let mut stream: Vec<u8> = Vec::new();
        send_rmp_value(&mut stream, datagram).await.unwrap();

        // Subscribe a function to the stream
        let c = Arc::new(Mutex::new(0));
        let c1 = c.clone();
        let mut cursor = Cursor::new(stream);
        bind_stream(&mut cursor, |_: Datagram| async {
            *c1.lock().await += 1;
        })
        .await
        .unwrap();

        // Equality check
        assert_eq!(*c.lock().await, 1);
    }
}
