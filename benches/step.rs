use criterion::{criterion_group, criterion_main, Criterion};
use particle_life::settings::Settings;
use particle_life::sim::Sim;
use rand::rngs::StdRng;
use rand::SeedableRng;

fn bench_step(c: &mut Criterion) {
    let mut rng = StdRng::from_seed([5; 32]);
    let mut sim = Sim::new(Settings::balanced(), &mut rng);

    c.bench_function("step (balanced)", |b| b.iter(|| sim.step(1600.0, 900.0)));
}

criterion_group!(benches, bench_step);
criterion_main!(benches);
