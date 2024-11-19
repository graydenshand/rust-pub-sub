/// This benchmark demonstartes that TCP connection is faster than a UDP connection under certain circumstances. Namely,
/// when sending 1 small message at a time, UDP sends each message over the wire individually while TCP implements some
/// buffering automatically.


use criterion::BenchmarkId;
use std::io::{Write, Read};
use tokio::runtime::Runtime;
use std::net;
use std::thread;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

const UDP_DATAGRAM_MAX_SIZE: usize = 4096;

fn tcp_send(stream: &mut net::TcpStream, buf: &[u8]) {
    stream.write(buf).expect("couldn't send message");
}

fn udp_send(socket: &mut net::UdpSocket, buf: &[u8]) {
    socket.send(buf).expect("couldn't send message");
}

fn tcp_send_values(stream: &mut net::TcpStream, n_values: u64) {
    let buf: Vec<[u8; 8]> = (0..n_values).map(|v| v.to_le_bytes()).collect();
    buf.chunks(UDP_DATAGRAM_MAX_SIZE / 8).for_each(|chunk| {
        let dg: Vec<u8> = chunk.iter().flat_map(|v| v.iter()) .copied().collect();
        tcp_send(stream, &dg);
    });
}

fn udp_send_values(socket: &mut net::UdpSocket, n_values: u64) {
    let buf: Vec<[u8; 8]> = (0..n_values).map(|v| v.to_le_bytes()).collect();
    buf.chunks(UDP_DATAGRAM_MAX_SIZE / 8).for_each(|chunk| {
        let dg: Vec<u8> = chunk.iter().flat_map(|v| v.iter()) .copied().collect();
        udp_send(socket, &dg);
    });
    
}

fn tcp_receive_values(addr: &str) -> thread::JoinHandle<()> {
    let tcp_listener = net::TcpListener::bind(addr).expect("couldn't bind to address");

    thread::spawn(move || {
        for result in tcp_listener.incoming() {
            let mut stream = result.expect("unable to accept connection");
            // thread::spawn(move || {
            let mut messages = 0;
            let mut buf = [0; 8];
            loop {
                let bytes_read = stream.read(&mut buf).expect("unable to read bytes from stream");
                if bytes_read == 0 {
                    break
                }
                let value = f64::from_le_bytes(buf);
                messages += 1;
            }
            // });
            
        }
    })
}

fn udp_receive_values(addr: &str) -> thread::JoinHandle<()> {
    let udp_listener = net::UdpSocket::bind(addr).expect("unable to bind to address");

    thread::spawn(move || {
        let mut buf = [0; UDP_DATAGRAM_MAX_SIZE];
        let mut values = 0;
        loop {
            let (bytes_read, src_addr) = udp_listener.recv_from(&mut buf).expect("no data received");
            if bytes_read == 0 {
                break
            }
            buf[0..bytes_read].chunks(8).for_each(|v| {
                let mut c = [0;8];
                c.copy_from_slice(v);
                let value = u64::from_le_bytes(c);
                // println!("udp_value: {:?}", value);
                values += 1;
            });            
        }
    })
}

fn network_streams(c: &mut Criterion) {
    let mut group: criterion::BenchmarkGroup<'_, criterion::measurement::WallTime> = c.benchmark_group("network_streams");

    let t = tcp_receive_values("127.0.0.1:36913");
    let u = udp_receive_values("127.0.0.1:36914");

    for scale in 0..7 {
        let n_values: u64 = 10_u64.pow(scale);
        let mut stream = net::TcpStream::connect("127.0.0.1:36913").expect("couldn't connect to remote addr");
        group.bench_with_input(BenchmarkId::new("Tcp", n_values), &n_values, 
            |b, i| b.iter(|| tcp_send_values(&mut stream, n_values)));

        let mut socket = net::UdpSocket::bind("127.0.0.1:36915").expect("unable to bind to address");
        // Connect to udp server address
        socket.connect("127.0.0.1:36914").expect("unable to connect to remote address");
        group.bench_with_input(BenchmarkId::new("Udp", n_values), &n_values, 
            |b, i| b.iter(|| udp_send_values(&mut socket, n_values)));

    }
    group.finish()
}

criterion_group!(benches, network_streams);
criterion_main!(benches);
