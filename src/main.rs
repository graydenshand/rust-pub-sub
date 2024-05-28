use std::net::{TcpListener, TcpStream};
use std::io::{ErrorKind, Read, Write};
use std::time::{Instant, Duration};
use std::thread;

fn handle_client(mut stream: TcpStream) {
    stream.set_read_timeout(Some(Duration::from_millis(500))).expect("set stream read timeout");
    let start = Instant::now();
    println!("Connection made");
    let mut bytes = [0; 8];

    let mut count = 0;
    loop {
        // let mut bytes = vec![0];
        let resp = stream.read_exact(&mut bytes);
        if let Some(e) = resp.err() {
            let end = Instant::now();
            println!("{} messages received in {}s", count, (end - start).as_millis() as f64 / 1000.0);
            if e.kind() == ErrorKind::UnexpectedEof {
                println!("Disconnected");
                return
            } else {
                panic!("Unexpected error {:?}", e);
            }
        }
        count += 1;
    }

    
}


fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:1996")?;

    // accept connections and process them serially
    for stream in listener.incoming() {
        thread::spawn(|| handle_client(stream.unwrap()));
    }
    Ok(())
}