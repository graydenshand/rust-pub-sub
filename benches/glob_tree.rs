
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rps::glob_tree::GlobTree;
use criterion::BenchmarkId;
use criterion::Throughput;

pub fn check_pattern_size(c: &mut Criterion) {
    let s = std::iter::repeat("a").take(200).collect::<String>();
    let mut group = c.benchmark_group("check_pattern_size");
    for i in (0..=100).step_by(10) {
        let mut t = GlobTree::new();        
        let mut pattern = std::iter::repeat("a*").take(i).collect::<String>();
        t.insert(&pattern);
        pattern.push('b');
        group.throughput(Throughput::Elements(i as u64,));
        group.bench_function( BenchmarkId::from_parameter(i), |b| b.iter(|| t.check(&s)));    
    }
}

pub fn check_string_size(c: &mut Criterion) {
    let mut t = GlobTree::new();        
    let mut pattern = "a*b".to_string();
    t.insert(&pattern);
    
    let mut group = c.benchmark_group("check_string_size");
    for i in (0..=100).step_by(10) {
        let s = std::iter::repeat("a").take(i).collect::<String>();    
        pattern.push('b');
        group.throughput(Throughput::Elements(i as u64,));
        group.bench_function(BenchmarkId::from_parameter(i), |b| b.iter(|| t.check(&s)));    
    }
}



criterion_group!(benches, check_pattern_size, check_string_size);
criterion_main!(benches);

