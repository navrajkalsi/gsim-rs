use criterion::Criterion;
use criterion::{criterion_group, criterion_main};
use gsim_rs::lexer::Lexer;
use gsim_rs::source::Source;

fn lexer(mut lexer: Lexer) {
    while let Some(_) = lexer.next() {}
}

fn criterion_benchmark(c: &mut Criterion) {
    let l = Lexer::new(Source::from_file("gcodes/adaptive.gcode").unwrap());
    c.bench_function("lexer", |b| b.iter(|| lexer(l.clone())));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
