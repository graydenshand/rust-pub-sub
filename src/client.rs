
use rmp_serde::{Serializer};
use rmpv::{Value};
use serde::Serialize;

use std::io::Write;
use std::net::TcpStream;


mod datagram;
use datagram::Message;
use std::io::BufWriter;

fn main() -> std::io::Result<()> {
    let mut i = 0;
    let mut stream = BufWriter::new(TcpStream::connect("127.0.0.1:1996")?);
    loop {
        let val = Value::from(i);
        // println!("{:?}", val);
        // encode::write_value(&mut stream, &val).unwrap();
        let message = Message::new("test", val);
        let mut buf = Vec::new();
        message.serialize(&mut Serializer::new(&mut buf)).unwrap();
        stream.write(&mut buf)?;
        i += 1;
    }
}
