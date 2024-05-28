use std::net::TcpStream;
use std::io::Read;
use std::io::Write;
use std::time::Duration;
use std::{thread, time};

fn main() -> std::io::Result<()> {

    let mut i = 0;
    let mut stream = TcpStream::connect("127.0.0.1:1996")?;
    loop {
        stream.write(&(i as i64).to_le_bytes())?;
        i +=1;
    }
}