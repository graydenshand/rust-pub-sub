use rmp_serde::to_vec;
use rmp_serde::{Deserializer, Serializer};
use rmpv::{decode, encode, Value};
use serde::Serialize;
use std::io::Read;
use std::io::Write;
use std::net::TcpStream;
use std::time::Duration;
use std::{thread, time};
mod datagram;
use datagram::Message;

fn main() -> std::io::Result<()> {
    let mut i = 0;
    let mut stream = TcpStream::connect("127.0.0.1:1996")?;
    loop {
        let val = Value::from(i);
        // println!("{:?}", val);
        // encode::write_value(&mut stream, &val).unwrap();
        let message = Message::new("test", val);
        let mut buf = Vec::new();
        message.serialize(&mut Serializer::new(&mut buf)).unwrap();
        stream.write_all(&mut buf)?;
        i += 1;
    }
}
