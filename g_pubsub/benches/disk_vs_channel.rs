/// A benchmark to compare the performance of communicating through disk compared with using a channel.
/// 
/// This is intended to emulate the core difference of a write ahead log architecture, and an architecture based on
/// channels.
/// 
/// The test entails measuring the interval between when a value is written and when a value is read from a
/// given media. The expectation is that the channel/file is already open on both sides.
/// 
/// My hypothesis is that writing to a disk will be an order of magnitude slower.

use tokio::time;
use tokio::fs::File;
use tokio::sync::broadcast;
use tokio;
use tokio::io::{AsyncWriteExt, AsyncReadExt};

async fn send_value_disk(mut file: tokio::fs::File, value: u8) -> time::Instant {
    file.write_u8(value).await.unwrap();
    time::Instant::now()
}

async fn receive_value_disk(mut file: File) -> time::Instant {
    loop {
        match file.read_u8().await {
            Ok(c) => break,
            Err(e) => continue
        }
    }
    time::Instant::now()
}

async fn send_value_channel<T>(channel: broadcast::Sender<T>, value: T) -> time::Instant {
    let _ = channel.send(value);
    time::Instant::now()
}

async fn receive_value_channel<T: Clone>(mut channel: broadcast::Receiver<T>) -> time::Instant {
    // Wait until a message is received over the channel and return the end time
    channel.recv().await.unwrap();
    time::Instant::now()
}

async fn measure_disk(path: &str) -> time::Duration {
    let file1 = File::create(path).await.unwrap();
    let file2 = File::open(path).await.unwrap();
    
    let future1 = tokio::spawn(async move {
        receive_value_disk(file2).await
    });

    let future2 = tokio::spawn(async move {
        send_value_disk(file1, 0).await
    });

    let (r1, r2) = tokio::join!(future1, future2);

    r1.unwrap() - r2.unwrap()
}

async fn measure_channel()  -> time::Duration {
    let (sender, receiver) = broadcast::channel::<u8>(1);

    let f1 = tokio::spawn(async move {
        receive_value_channel(receiver).await
    });

    let f2 = tokio::spawn(async move {
        send_value_channel(sender, 0).await
    });
    
    let (r1, r2) = tokio::join!(f1, f2);

    r1.unwrap() - r2.unwrap()
}

#[tokio::main]
async fn main() {
    let n_runs = 10000;
    let mut channel_runs = vec![];
    for _ in 0..n_runs {
        let channel_result = measure_channel().await;
        channel_runs.push(channel_result);
    }
    let sum: u128 = channel_runs.iter().map(|d| d.as_nanos()).sum();
    let  mean = time::Duration::from_nanos( (sum / n_runs) as u64);
    println!("Channel result {mean:?}");

    let mut disk_runs = vec![];
    for _ in 0..n_runs {
        let channel_result = measure_disk("./.disk-vs-channel-bench").await;
        disk_runs.push(channel_result);
    }
    let sum: u128 = disk_runs.iter().map(|d| d.as_nanos()).sum();
    let  mean = time::Duration::from_nanos( (sum / n_runs) as u64);

    // let disk_result = measure_disk("./.disk-vs-channel-bench").await;
    println!("Disk result {mean:?}");
}

