use criterion::Criterion;
use criterion::{criterion_group, criterion_main};
use gsim_rs::lexer::Lexer;
use gsim_rs::parser::Parser;
use gsim_rs::source::Source;

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("parser", |b| {
        b.iter_batched(
            || {
                Parser::new(Lexer::new(
                    Source::from_file("gcodes/adaptive.gcode").unwrap(),
                ))
            },
            |mut p| {
                while let Some(block) = p.next() {
                    let _ = std::hint::black_box(block);
                }
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
