use std::net::{TcpListener, TcpStream};
use std::io::Read;
use std::io::Write;
use std::time::Duration;
use std::{thread, time};

fn handle_response(mut stream: TcpStream) {
    let mut bytes = [0; 128];
    stream.read(&mut bytes).unwrap();
    println!("Connection made: {:?}", bytes);
    
}


fn main() -> std::io::Result<()> {

    // accept connections and process them serially
    // for stream in listener.incoming() {
    //     handle_response(stream?);
    // }
    let mut i = 0;
    let mut stream = TcpStream::connect("127.0.0.1:1996")?;
    stream.set_write_timeout(Some(Duration::from_millis(500))).unwrap();
    loop {
        stream.write(&(i as i64).to_le_bytes())?;
        // stream.read(&mut [0; 128])?;
        i +=1;
        // thread::sleep(time::Duration::from_micros(100));
    }
}