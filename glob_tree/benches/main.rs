use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use glob_tree::GlobTree;
use regex::Regex;

// ============================================================================
// INSERT BENCHMARKS
// ============================================================================

pub fn insert_linear_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_linear_patterns");
    for count in [10, 50, 100, 500, 1000] {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            b.iter(|| {
                let mut t = GlobTree::new();
                for i in 0..count {
                    t.insert(&format!("pattern_{}", i));
                }
                black_box(t);
            });
        });
    }
    group.finish();
}

pub fn insert_overlapping_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_overlapping_patterns");
    for count in [10, 50, 100, 500] {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            b.iter(|| {
                let mut t = GlobTree::new();
                // All patterns share "prefix/"
                for i in 0..count {
                    t.insert(&format!("prefix/pattern_{}", i));
                }
                black_box(t);
            });
        });
    }
    group.finish();
}

pub fn insert_wildcard_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_wildcard_patterns");
    for count in [10, 50, 100, 500] {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            b.iter(|| {
                let mut t = GlobTree::new();
                for i in 0..count {
                    t.insert(&format!("*.{}", i));
                }
                black_box(t);
            });
        });
    }
    group.finish();
}

// ============================================================================
// CHECK BENCHMARKS - BASIC
// ============================================================================

pub fn check_literal_match(c: &mut Criterion) {
    let mut t = GlobTree::new();
    t.insert("exact/path/to/file.txt");

    c.bench_function("check_literal_match", |b| {
        b.iter(|| black_box(t.check("exact/path/to/file.txt")))
    });
}

pub fn check_literal_no_match(c: &mut Criterion) {
    let mut t = GlobTree::new();
    t.insert("exact/path/to/file.txt");

    c.bench_function("check_literal_no_match", |b| {
        b.iter(|| black_box(t.check("different/path/to/file.txt")))
    });
}

pub fn check_wildcard_suffix(c: &mut Criterion) {
    let mut t = GlobTree::new();
    t.insert("logs/*.txt");

    c.bench_function("check_wildcard_suffix", |b| {
        b.iter(|| black_box(t.check("logs/application.txt")))
    });
}

pub fn check_wildcard_prefix(c: &mut Criterion) {
    let mut t = GlobTree::new();
    t.insert("*.log");

    c.bench_function("check_wildcard_prefix", |b| {
        b.iter(|| black_box(t.check("application.log")))
    });
}

pub fn check_wildcard_middle(c: &mut Criterion) {
    let mut t = GlobTree::new();
    t.insert("src/*/main.rs");

    c.bench_function("check_wildcard_middle", |b| {
        b.iter(|| black_box(t.check("src/module/main.rs")))
    });
}

pub fn check_single_char_wildcard(c: &mut Criterion) {
    let mut t = GlobTree::new();
    t.insert("file?.txt");

    c.bench_function("check_single_char_wildcard", |b| {
        b.iter(|| black_box(t.check("file1.txt")))
    });
}

// ============================================================================
// CHECK BENCHMARKS - SCALING
// ============================================================================

pub fn check_string_length_scaling(c: &mut Criterion) {
    let mut t = GlobTree::new();
    t.insert("prefix*suffix");

    let mut group = c.benchmark_group("check_string_length_scaling");
    for len in [10, 50, 100, 500, 1000] {
        let s = format!("prefix{}suffix", "x".repeat(len));
        group.throughput(Throughput::Bytes(s.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(len), &s, |b, s| {
            b.iter(|| black_box(t.check(s)));
        });
    }
    group.finish();
}

pub fn check_many_patterns_hit(c: &mut Criterion) {
    let mut group = c.benchmark_group("check_many_patterns_hit");

    for count in [10, 50, 100, 500, 1000] {
        let mut t = GlobTree::new();
        for i in 0..count {
            t.insert(&format!("pattern/{}", i));
        }

        // Search for the LAST pattern (worst case for linear search)
        let target = format!("pattern/{}", count - 1);

        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), &target, |b, target| {
            b.iter(|| black_box(t.check(target)));
        });
    }
    group.finish();
}

pub fn check_many_patterns_miss(c: &mut Criterion) {
    let mut group = c.benchmark_group("check_many_patterns_miss");

    for count in [10, 50, 100, 500, 1000] {
        let mut t = GlobTree::new();
        for i in 0..count {
            t.insert(&format!("pattern/{}", i));
        }

        let target = "nomatch/pattern";

        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), &target, |b, target| {
            b.iter(|| black_box(t.check(target)));
        });
    }
    group.finish();
}

pub fn check_overlapping_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("check_overlapping_patterns");

    for count in [10, 50, 100, 500] {
        let mut t = GlobTree::new();
        // Create overlapping patterns: "a", "ab", "abc", "abcd", ...
        for i in 1..=count {
            t.insert(&"a".repeat(i));
        }

        // Check against longest pattern
        let target = "a".repeat(count);

        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), &target, |b, target| {
            b.iter(|| black_box(t.check(target)));
        });
    }
    group.finish();
}

// ============================================================================
// CONTAINS BENCHMARKS
// ============================================================================

pub fn contains_hit(c: &mut Criterion) {
    let mut t = GlobTree::new();
    for i in 0..1000 {
        t.insert(&format!("pattern/{}", i));
    }

    c.bench_function("contains_hit", |b| {
        b.iter(|| black_box(t.contains("pattern/500")))
    });
}

pub fn contains_miss(c: &mut Criterion) {
    let mut t = GlobTree::new();
    for i in 0..1000 {
        t.insert(&format!("pattern/{}", i));
    }

    c.bench_function("contains_miss", |b| {
        b.iter(|| black_box(t.contains("pattern/9999")))
    });
}

pub fn contains_partial_match(c: &mut Criterion) {
    let mut t = GlobTree::new();
    t.insert("exact/path/to/file");

    c.bench_function("contains_partial_match", |b| {
        b.iter(|| black_box(t.contains("exact/path")))
    });
}

// ============================================================================
// LIST BENCHMARKS
// ============================================================================

pub fn list_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_patterns");

    for count in [10, 50, 100, 500, 1000] {
        let mut t = GlobTree::new();
        for i in 0..count {
            t.insert(&format!("pattern/{}", i));
        }

        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), &t, |b, t| {
            b.iter(|| black_box(t.list()));
        });
    }
    group.finish();
}

pub fn list_overlapping_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_overlapping_patterns");

    for count in [10, 50, 100, 500] {
        let mut t = GlobTree::new();
        // All share "prefix/"
        for i in 0..count {
            t.insert(&format!("prefix/pattern/{}", i));
        }

        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), &t, |b, t| {
            b.iter(|| black_box(t.list()));
        });
    }
    group.finish();
}

// ============================================================================
// REMOVE BENCHMARKS
// ============================================================================

pub fn remove_pattern(c: &mut Criterion) {
    c.bench_function("remove_pattern", |b| {
        b.iter_batched(
            || {
                let mut t = GlobTree::new();
                for i in 0..100 {
                    t.insert(&format!("pattern/{}", i));
                }
                t
            },
            |mut t| {
                black_box(t.remove("pattern/50").unwrap());
                t
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

// ============================================================================
// REALISTIC WORKLOAD BENCHMARKS
// ============================================================================

pub fn realistic_file_patterns(c: &mut Criterion) {
    let mut t = GlobTree::new();
    t.insert("*.rs");
    t.insert("*.toml");
    t.insert("src/*.rs");
    t.insert("tests/*.rs");
    t.insert("benches/*.rs");
    t.insert(".git/*");
    t.insert("target/*");
    t.insert("*.log");

    let mut group = c.benchmark_group("realistic_file_patterns");

    let test_cases = vec![
        ("main.rs", true),
        ("lib.rs", true),
        ("src/main.rs", true),
        ("tests/integration_test.rs", true),
        ("target/debug", true),
        ("README.md", false),
        ("docs/guide.txt", false),
    ];

    for (path, _matches) in test_cases {
        group.bench_with_input(BenchmarkId::from_parameter(path), path, |b, path| {
            b.iter(|| black_box(t.check(path)));
        });
    }
    group.finish();
}

pub fn realistic_topic_patterns(c: &mut Criterion) {
    let mut t = GlobTree::new();
    t.insert("metrics/cpu/*");
    t.insert("metrics/memory/*");
    t.insert("logs/*/error");
    t.insert("logs/*/info");
    t.insert("events/*/*");
    t.insert("notifications/user/*");

    let mut group = c.benchmark_group("realistic_topic_patterns");

    let test_cases = vec![
        "metrics/cpu/usage",
        "metrics/memory/available",
        "logs/app1/error",
        "logs/app2/info",
        "events/payment/completed",
        "notifications/user/123",
    ];

    for topic in test_cases {
        group.bench_with_input(BenchmarkId::from_parameter(topic), topic, |b, topic| {
            b.iter(|| black_box(t.check(topic)));
        });
    }
    group.finish();
}

// ============================================================================
// REGEX COMPARISON BENCHMARKS
// ============================================================================

pub fn compare_single_pattern_vs_regex(c: &mut Criterion) {
    let mut group = c.benchmark_group("compare_single_pattern_vs_regex");

    let mut t = GlobTree::new();
    t.insert("src/*.rs");

    let re = Regex::new(r"^src/[^/]*\.rs$").unwrap();
    let test_str = "src/main.rs";

    group.bench_function("glob_tree", |b| {
        b.iter(|| black_box(t.check(test_str)));
    });

    group.bench_function("regex", |b| {
        b.iter(|| black_box(re.is_match(test_str)));
    });

    group.finish();
}

pub fn compare_many_patterns_vs_regex(c: &mut Criterion) {
    let mut group = c.benchmark_group("compare_many_patterns_vs_regex");

    for count in [10, 50, 100] {
        let mut t = GlobTree::new();
        let mut regex_patterns = Vec::new();

        for i in 0..count {
            let pattern = format!("pattern/{}/*", i);
            t.insert(&pattern);
            // Convert glob to regex (simplified)
            let regex_pattern = format!(r"^pattern/{}/.*$", i);
            regex_patterns.push(regex_pattern);
        }

        let combined_regex = Regex::new(&format!("^({})$", regex_patterns.join("|"))).unwrap();

        let test_str = format!("pattern/{}/file", count / 2);

        group.bench_with_input(BenchmarkId::new("glob_tree", count), &test_str, |b, s| {
            b.iter(|| black_box(t.check(s)))
        });

        group.bench_with_input(BenchmarkId::new("regex", count), &test_str, |b, s| {
            b.iter(|| black_box(combined_regex.is_match(s)))
        });
    }

    group.finish();
}

pub fn compare_non_overlapping_patterns_vs_regex(c: &mut Criterion) {
    let mut group = c.benchmark_group("compare_non_overlapping_patterns_vs_regex");

    for count in [10, 50, 100] {
        let mut t = GlobTree::new();
        let mut regex_patterns = Vec::new();

        // Create completely different patterns with no shared prefixes
        for i in 0..count {
            let pattern = format!("route_{}/*", i);
            t.insert(&pattern);
            let regex_pattern = format!(r"^route_{}/.*$", i);
            regex_patterns.push(regex_pattern);
        }

        let combined_regex = Regex::new(&regex_patterns.join("|")).unwrap();

        let test_str = format!("route_{}/data", count / 2);

        group.bench_with_input(BenchmarkId::new("glob_tree", count), &test_str, |b, s| {
            b.iter(|| black_box(t.check(s)))
        });

        group.bench_with_input(BenchmarkId::new("regex", count), &test_str, |b, s| {
            b.iter(|| black_box(combined_regex.is_match(s)))
        });
    }

    group.finish();
}

// ============================================================================
// CRITERION GROUPS
// ============================================================================

criterion_group!(
    insert_benches,
    insert_linear_patterns,
    insert_overlapping_patterns,
    insert_wildcard_patterns,
);

criterion_group!(
    check_basic_benches,
    check_literal_match,
    check_literal_no_match,
    check_wildcard_suffix,
    check_wildcard_prefix,
    check_wildcard_middle,
    check_single_char_wildcard,
);

criterion_group!(
    check_scaling_benches,
    check_string_length_scaling,
    check_many_patterns_hit,
    check_many_patterns_miss,
    check_overlapping_patterns,
);

criterion_group!(
    contains_benches,
    contains_hit,
    contains_miss,
    contains_partial_match,
);

criterion_group!(list_benches, list_patterns, list_overlapping_patterns,);

criterion_group!(remove_benches, remove_pattern,);

criterion_group!(
    realistic_benches,
    realistic_file_patterns,
    realistic_topic_patterns,
);

criterion_group!(
    comparison_benches,
    compare_single_pattern_vs_regex,
    compare_many_patterns_vs_regex,
    compare_non_overlapping_patterns_vs_regex,
);

criterion_main!(
    insert_benches,
    check_basic_benches,
    check_scaling_benches,
    contains_benches,
    list_benches,
    remove_benches,
    realistic_benches,
    comparison_benches,
);
