use criterion::{black_box, criterion_group, criterion_main, Criterion};
use kubera_core::sync::signal;
use tokio::runtime::Runtime;

fn benchmark_signal_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("signal_creation", |b| {
        b.iter(|| {
            let (_tx, _rx) = signal::<i32>();
        });
    });

    c.bench_function("signal_set_get", |b| {
        let (tx, rx) = signal::<i32>();
        b.to_async(&rt).iter(|| async {
            tx.set(black_box(42)).await;
            black_box(rx.get().await);
        });
    });

    c.bench_function("signal_multiple_receivers", |b| {
        let (tx, rx) = signal::<i32>();
        let receivers: Vec<_> = (0..10).map(|_| rx.clone()).collect();

        b.to_async(&rt).iter(|| async {
            tx.set(black_box(42)).await;
            for rx in &receivers {
                black_box(rx.get().await);
            }
        });
    });
}

fn benchmark_networking_types(c: &mut Criterion) {
    use kubera_core::net::{Hostname, Port};

    c.bench_function("port_creation", |b| {
        b.iter(|| Port::new(black_box(8080)));
    });

    c.bench_function("hostname_creation", |b| {
        b.iter(|| Hostname::new(black_box("api.example.com")));
    });

    c.bench_function("hostname_case_insensitive_comparison", |b| {
        let h1 = Hostname::new("API.EXAMPLE.COM");
        let h2 = Hostname::new("api.example.com");
        b.iter(|| black_box(h1 == h2));
    });

    c.bench_function("hostname_ends_with", |b| {
        let hostname = Hostname::new("api.service.example.com");
        let suffix = Hostname::new("example.com");
        b.iter(|| black_box(hostname.ends_with(&suffix)));
    });
}

fn benchmark_serialization(c: &mut Criterion) {
    use kubera_core::net::{Hostname, Port};
    use serde_json;

    let port = Port::new(8080);
    let hostname = Hostname::new("api.example.com");

    c.bench_function("port_json_serialize", |b| {
        b.iter(|| black_box(serde_json::to_string(&port).unwrap()));
    });

    c.bench_function("hostname_json_serialize", |b| {
        b.iter(|| black_box(serde_json::to_string(&hostname).unwrap()));
    });

    let port_json = serde_json::to_string(&port).unwrap();
    let hostname_json = serde_json::to_string(&hostname).unwrap();

    c.bench_function("port_json_deserialize", |b| {
        b.iter(|| black_box(serde_json::from_str::<Port>(&port_json).unwrap()));
    });

    c.bench_function("hostname_json_deserialize", |b| {
        b.iter(|| black_box(serde_json::from_str::<Hostname>(&hostname_json).unwrap()));
    });
}

criterion_group!(
    benches,
    benchmark_signal_operations,
    benchmark_networking_types,
    benchmark_serialization
);
criterion_main!(benches);
