use criterion::{black_box, criterion_group, criterion_main, Criterion};

use blackbox_core::types::LogLine;
use blackbox_daemon::scanners::drain::{get_error_clusters, ingest_line, new_drain_state};

fn generate_logs(n: usize) -> Vec<LogLine> {
    let templates: Vec<Box<dyn Fn(usize) -> String>> = vec![
        Box::new(|i| format!("error: connection refused to 192.168.{}.{} port {}", i % 256, i % 10, 8000 + i % 1000)),
        Box::new(|i| format!("warn: disk space low on /dev/sda{}: {}%", i % 10, 80 + i % 20)),
        Box::new(|i| format!("fatal: unhandled exception in module {} at line {}", i % 100, i)),
        Box::new(|i| format!("info: request {} completed in {}ms", i, i % 500)),
        Box::new(|i| format!("error: timeout connecting to 10.0.{}.{}:8080", i % 256, i % 256)),
        Box::new(|i| format!("debug: user {} logged in from {} with session {}", i % 1000, i % 256, i)),
        Box::new(|i| format!("error: database query failed: SELECT * FROM users WHERE id = {}", i)),
        Box::new(|i| format!("warn: cache miss for key {} after {}ms", i, i % 100)),
        Box::new(|i| format!("info: worker {} started job {} at {}", i % 50, i, i % 24)),
        Box::new(|i| format!("error: file not found: /tmp/data/file_{}.txt", i)),
    ];

    (0..n)
        .map(|i| {
            let text = templates[i % templates.len()](i);
            LogLine {
                text,
                timestamp_ms: 1000 + i as u64,
                source_terminal: Some("bench".into()),
            }
        })
        .collect()
}

fn bench_drain_ingest(c: &mut Criterion) {
    let logs_1k = generate_logs(1_000);
    let logs_10k = generate_logs(10_000);

    c.bench_function("drain_ingest_1k", |b| {
        b.iter(|| {
            let state = new_drain_state();
            for log in logs_1k.iter() {
                ingest_line(black_box(&state), black_box(log));
            }
        })
    });

    c.bench_function("drain_ingest_10k", |b| {
        b.iter(|| {
            let state = new_drain_state();
            for log in logs_10k.iter() {
                ingest_line(black_box(&state), black_box(log));
            }
        })
    });

    // Benchmark query performance after ingestion
    let state = new_drain_state();
    for log in logs_10k.iter() {
        ingest_line(&state, log);
    }

    c.bench_function("drain_get_clusters_10k", |b| {
        b.iter(|| {
            let _ = get_error_clusters(black_box(&state), 100, None);
        })
    });
}

criterion_group!(benches, bench_drain_ingest);
criterion_main!(benches);
