// use std::net::{TcpListener, TcpStream};
use std::io::{ErrorKind, Read, Write};
use std::time::{Instant, Duration};
use std::thread;
use tokio::net::{TcpListener, TcpStream};
use std::error::Error;
use tokio::io::{self, AsyncReadExt, Interest};

mod datagram;
mod subscription_tree;

async fn process_socket(stream: TcpStream) {
    let start = Instant::now();
    println!("Connection made");
    let mut bytes = [0; 8];

    let mut count = 0;

    loop {
        match stream.try_read(&mut bytes) {
            Ok(bytes_written) => {
                if bytes_written == 0 {
                    let end = Instant::now();
                    println!("{} messages received in {}s", count, (end - start).as_millis() as f64 / 1000.0);
                    println!("Disconnected");
                    return
                }
                count += 1;
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                continue;
            }
            Err(e) => {
                panic!("{:?}", e)
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind("127.0.0.1:1996").await?;

    loop {
        let (socket, addr) = listener.accept().await?;
        tokio::spawn(async move {
            process_socket(socket).await;
        });
        
    }
}
