use std::borrow::BorrowMut;
// use std::net::{TcpListener, TcpStream};
use rmp_serde;
use rmp_serde::{Deserializer, Serializer};
use rmpv::{decode, Value};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::io::{ErrorKind, Read, Write};
use std::thread;
use std::time::{Duration, Instant};
use tokio;
use tokio::io::AsyncWriteExt;
use tokio::io::{self, AsyncReadExt, Interest};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::stream;
use tokio::time::timeout;

mod datagram;
mod subscription_tree;
use bytes::{buf::Reader, Buf, BufMut, Bytes, BytesMut};
use datagram::Message;

pub struct MessageReader {
    reader: OwnedReadHalf,
    buffer: BytesMut,
}

impl MessageReader {
    pub fn new(stream: OwnedReadHalf) -> MessageReader {
        MessageReader {
            reader: stream,
            buffer: BytesMut::with_capacity(4096),
        }
    }

    async fn receive_loop(&mut self) -> Result<(), Box<dyn Error>> {
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
            }

            count += 1;
        }
    }

    fn parse_value(&mut self) -> (Option<Message>, usize) {
        let buf = &mut &self.buffer[..];
        let start_len = buf.len();
        // let v= decode::value::read_value(buf).ok();
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
        self.writer.write_all(&mut buf).await?;
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind("127.0.0.1:1996").await?;

    loop {
        let (stream, _) = listener.accept().await?;
        println!("Connection made");
        let (r, w) = stream.into_split();
        let mut reader = MessageReader::new(r);
        tokio::spawn(async move {
            _ = reader.receive_loop().await;
        });

        let mut writer = MessageWriter::new(w);
        tokio::spawn(async move {
            _ = writer
                .send(Message::new("test", Value::Boolean(true)))
                .await;
        });
    }
}
