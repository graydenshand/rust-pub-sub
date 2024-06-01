use std::borrow::BorrowMut;
// use std::net::{TcpListener, TcpStream};
use std::io::{ErrorKind, Read, Write};
use std::time::{Instant, Duration};
use std::thread;
use tokio::net::{TcpListener, TcpStream};
use std::error::Error;
use tokio::io::{self, AsyncReadExt, Interest};
use rmp_serde;
use rmpv::{Value, decode};
use tokio::time::timeout;

mod datagram;
mod subscription_tree;
use bytes::{BytesMut, Bytes, Buf, BufMut, buf::Reader};


pub struct Connection {
    stream: TcpStream,
    buffer: BytesMut,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Connection {
        Connection {
            stream,
            buffer: BytesMut::with_capacity(4096),
        }
    }

    fn parse_value(&mut self) -> (Option<Value>, usize) {
        // println!("{:?}", &self.buffer[..]);
        let buf = &mut &self.buffer[..];
        // let buf = self.buffer.chunk_mut();
        let start_len = buf.len();
        let v= decode::value::read_value(buf).ok();
        let end_len = buf.len();
        // if let Some(value) = v {
        //     // self.buffer.advance();
        // };
        // println!("{:?} - bytes read: {}", buf, start_len - end_len);
        
        (v, (start_len - end_len))
    }

    pub async fn read_value(&mut self)
        -> Result<Option<Value>, Box<dyn Error>>
    {   
        loop {
            // Attempt to parse a frame from the buffered data. If
            // enough data has been buffered, the frame is
            // returned.
            let (value, bytes_read) = self.parse_value();
            if let Some(v) = value {
                self.buffer.advance(bytes_read);
                return Ok(Some(v));
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

async fn process_socket(stream: TcpStream) -> Result<(), Box<dyn Error>> {
    let start = Instant::now();
    println!("Connection made");
    let mut connection = Connection::new(stream);

    let mut count = 0;

    loop {
        let value = connection.read_value().await;
        if value.is_err() || value.as_ref().unwrap().is_none() {
            let end = Instant::now();
            let seconds = (end - start).as_millis() as f64 / 1000.0;
            println!("{} messages received in {}s - {} m/s", count, seconds, (count as f64 / seconds).round());
            println!("Disconnected");
            return Ok(())
        } else {
            // Do something with the value
            // println!("{:?}", value?.unwrap());
        }

        count += 1;
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind("127.0.0.1:1996").await?;

    loop {
        let (socket, addr) = listener.accept().await?;
        tokio::spawn(async move {
            _ = process_socket(socket).await;
        });
        
    }
}
