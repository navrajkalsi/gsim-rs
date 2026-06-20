use criterion::Criterion;
use criterion::{criterion_group, criterion_main};
use gsim_rs::config::Point;
use gsim_rs::geometry::StockInstance;

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("geometry", |b| {
        b.iter(|| {
            let _ = std::hint::black_box(StockInstance::stock(Point {
                x: 500.0,
                y: 500.0,
                z: 500.0,
            }));
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
