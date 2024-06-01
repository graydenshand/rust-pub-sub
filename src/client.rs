use std::net::TcpStream;
use std::io::Read;
use std::io::Write;
use std::time::Duration;
use std::{thread, time};
use rmp_serde::to_vec;
use rmpv::{Value, encode, decode};



fn main() -> std::io::Result<()> {

    let mut i = 0;
    let mut stream = TcpStream::connect("127.0.0.1:1996")?;
    loop {
        let val = Value::from(i);
        // println!("{:?}", val);
        encode::write_value(&mut stream, &val).unwrap();
        i +=1;
    }
    Ok(())
}