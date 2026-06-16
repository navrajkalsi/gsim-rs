use criterion::Criterion;
use criterion::{criterion_group, criterion_main};
use gsim_rs::config::{Point, Unit};
use gsim_rs::interpreter::Interpreter;
use gsim_rs::lexer::Lexer;
use gsim_rs::machine::Machine;
use gsim_rs::parser::Parser;
use gsim_rs::source::Source;

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("interpreter", |b| {
        b.iter_batched(
            || {
                Interpreter::new(
                    Parser::new(Lexer::new(
                        Source::from_file("gcodes/adaptive.gcode").unwrap(),
                    )),
                    Machine::new(
                        Unit::Metric,
                        Point::new(0.0, 0.0, 0.0),
                        Point::new(0.0, 0.0, 0.0),
                    ),
                )
            },
            |mut i| {
                while let Some(summary) = i.execute().unwrap() {
                    let _ = std::hint::black_box(summary);
                }
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
