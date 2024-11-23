use rmp_serde::Serializer;
use rmpv::Value;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use rmp_serde;

use bytes::{Buf, BytesMut};
use std::fmt;

use tokio_util::codec::{Decoder, Encoder};

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
    /// Subscribe to a topic
    Unsubscribe { pattern: String },
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
            Self::Unsubscribe { pattern } => {
                write!(f, "UNSUBSCRIBE - {pattern}")
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

/// A Codec for sending and receiving byte-encoded msgpack payloads
///
/// This is intended to produce or consume a stream of type T where T is a type that
/// can be serialized by msgpack.
#[derive(Debug, Clone)]
pub struct MsgPackCodec<T> {
    _marker: std::marker::PhantomData<T>,
}
impl<T> MsgPackCodec<T> {
    pub fn new() -> MsgPackCodec<T> {
        MsgPackCodec::<T> {
            _marker: std::marker::PhantomData,
        }
    }
}
impl<T: DeserializeOwned> Decoder for MsgPackCodec<T> {
    type Item = T;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // Attempt to parse a frame from the buffered data. If
        // enough data has been buffered, the frame is
        // returned.
        let buf = &mut &src[..];
        let start_len = buf.len();
        let command = rmp_serde::decode::from_read::<&mut &[u8], T>(buf);
        let bytes_read = start_len - buf.len();

        match command {
            Ok(c) => {
                src.advance(bytes_read);
                Ok(Some(c))
            }
            Err(e) => {
                // Not able to parse a value from the data currently in the stream
                match e {
                    rmp_serde::decode::Error::LengthMismatch(_)
                    | rmp_serde::decode::Error::InvalidMarkerRead(_)
                    | rmp_serde::decode::Error::InvalidDataRead(_) => Ok(None),
                    _ => Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        e.to_string(),
                    )),
                }
            }
        }
    }
}
impl<T: Serialize> Encoder<T> for MsgPackCodec<T> {
    type Error = std::io::Error;

    fn encode(&mut self, item: T, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let mut buf = Vec::new();
        match item.serialize(&mut Serializer::new(&mut buf)) {
            Ok(_) => {
                dst.extend_from_slice(&buf);
                Ok(())
            }
            Err(e) => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::io::Cursor;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tokio_util::codec::{FramedRead, FramedWrite};

    #[tokio::test]
    async fn sends_and_receives() {
        // Create a sink, and wrap with our MsgPackCodec
        let stream = Vec::new();
        let codec = MsgPackCodec::<Datagram>::new();
        let mut writer = FramedWrite::new(stream, codec.clone());

        // Send a command
        let topic = "test".into();
        let value = Value::from("test");
        let message = Message { topic, value };
        let command = Command::Publish { message };
        let datagram = Datagram::new(command, "sender");
        let d1 = datagram.clone();
        writer.send(datagram).await.unwrap();

        // Claim the stream from the writer
        let stream = writer.into_inner();
        let mut reader = FramedRead::new(Cursor::new(stream), codec);
        // // Receive the command
        match reader.next().await {
            None => panic!("value was null"),
            Some(r) => match r {
                Err(e) => panic!("value was error - {e}"),
                Ok(d2) => {
                    assert_eq!(d1, d2)
                }
            },
        }
    }

    #[tokio::test]
    async fn receives_bad_data() {
        // Create a stream with some arbitrary bytes
        let stream = vec![12, 120, 27];
        // Stack the MsgPackCodec on top of the stream
        let codec = MsgPackCodec::<Datagram>::new();
        let mut reader = FramedRead::new(Cursor::new(stream), codec);
        // Receive the command
        let v = reader.next().await.unwrap();
        // Assert an error is produced
        assert!(v.is_err());
    }
}
