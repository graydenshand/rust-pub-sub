use tokio;
use criterion::BenchmarkId;
use criterion::Criterion;
use criterion::{criterion_group, criterion_main};

use tokio::runtime::Runtime;

// Here we have an async function to benchmark
async fn do_something(size: usize) {
    // Do something async with the size
    tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
}

fn from_elem(c: &mut Criterion) {
    let size: usize = 1024;

    c.bench_with_input(BenchmarkId::new("from_elem", size), &size, |b, &s| {
        // Insert a call to `to_async` to convert the bencher to async mode.
        // The timing loops are the same as with the normal bencher.
        let runtime = Runtime::new().unwrap();
        b.to_async(runtime).iter(|| do_something(s));
    });
}

/// Needed functions
/// - run server
/// - run client
/// - run n clients (read only, N is independent variable)
/// - run 1 client (writer)
/// 
/// Metric
/// - latency between publish and receive (dependent variable)

criterion_group!(benches, from_elem);
criterion_main!(benches);
