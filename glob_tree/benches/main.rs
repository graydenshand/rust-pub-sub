use std::vec;

use criterion::BenchmarkId;
use criterion::Throughput;
use criterion::{criterion_group, criterion_main, Criterion};
use glob_tree::GlobTree;
use regex::Regex;

pub fn check_pattern_size(c: &mut Criterion) {
    let s = std::iter::repeat("a").take(200).collect::<String>();
    let mut group = c.benchmark_group("check_pattern_size");
    for i in (0..=100).step_by(10) {
        let mut t = GlobTree::new();
        let mut pattern = std::iter::repeat("a*").take(i).collect::<String>();
        t.insert(&pattern);
        pattern.push('b');
        group.throughput(Throughput::Elements(i as u64));
        group.bench_function(BenchmarkId::from_parameter(i), |b| b.iter(|| t.check(&s)));
    }
}

pub fn check_string_size(c: &mut Criterion) {
    let mut t = GlobTree::new();
    let pattern = "a*b".to_string();
    t.insert(&pattern);

    let mut group = c.benchmark_group("check_string_size");
    for i in (0..=100).step_by(10) {
        let mut s = std::iter::repeat("a").take(i).collect::<String>();
        s.push('b');
        group.throughput(Throughput::Elements(i as u64));
        group.bench_function(BenchmarkId::from_parameter(i), |b| b.iter(|| t.check(&s)));
    }
}

pub fn check_pattern_size_regex(c: &mut Criterion) {
    let s = std::iter::repeat("a").take(200).collect::<String>();
    let mut group = c.benchmark_group("check_pattern_size_regex");
    for i in (0..=100).step_by(10) {
        let mut pattern = std::iter::repeat("a.*").take(i).collect::<String>();
        pattern.push('b');
        let re = Regex::new(&pattern).unwrap();
        group.throughput(Throughput::Elements(i as u64));
        group.bench_function(BenchmarkId::from_parameter(i), |b| {
            b.iter(|| Regex::find(&re, &s))
        });
    }
}

pub fn check_string_size_regex(c: &mut Criterion) {
    let pattern = r"a.*b";
    let re = Regex::new(pattern).unwrap();

    let mut group = c.benchmark_group("check_string_size_regex");
    for i in (0..=100).step_by(10) {
        let mut s = std::iter::repeat("a").take(i).collect::<String>();
        s.push('b');
        group.throughput(Throughput::Elements(i as u64));
        group.bench_function(BenchmarkId::from_parameter(i), |b| {
            b.iter(|| Regex::find(&re, &s))
        });
    }
}

pub fn check_many_patterns(c: &mut Criterion) {
    let mut t = GlobTree::new();

    let mut group = c.benchmark_group("check_many_patterns");
    for i in 0..=100 {
        let mut pattern = std::iter::repeat("a").take(i).collect::<String>();
        pattern.push('b');
        t.insert(&pattern);
        let mut s = std::iter::repeat("a").take(i * 2).collect::<String>();
        s.push('b');
        if i % 10 == 0 {
            group.throughput(Throughput::Elements(i as u64));
            group.bench_function(BenchmarkId::from_parameter(i), |b| b.iter(|| t.check(&s)));
        }
    }
}

pub fn check_many_patterns_regex(c: &mut Criterion) {
    let mut patterns = vec![];

    let mut group = c.benchmark_group("check_many_patterns_regex");
    for i in 0..=100 {
        let mut pattern = std::iter::repeat("a").take(i).collect::<String>();
        pattern.push('b');
        patterns.push(pattern);
        let combined_pattern = Regex::new(
            &patterns
                .iter()
                .map(|p| format!("({p})"))
                .collect::<Vec<String>>()
                .join("|"),
        )
        .unwrap();
        let mut s = std::iter::repeat("a").take(i * 2).collect::<String>();
        s.push('b');
        if i % 10 == 0 {
            group.throughput(Throughput::Elements(i as u64));
            group.bench_function(BenchmarkId::from_parameter(i), |b| {
                b.iter(|| Regex::captures(&combined_pattern, &s))
            });
        }
    }
}

// criterion_group!(benches, check_pattern_size, check_string_size, check_pattern_size_regex, check_string_size_regex);
// criterion_group!(benches, check_pattern_size_regex, check_string_size_regex);
criterion_group!(benches, check_many_patterns_regex);
criterion_main!(benches);
