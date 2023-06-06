use criterion::measurement::WallTime;
use criterion::{criterion_group, criterion_main, BenchmarkGroup, Criterion};
use particle_life::settings::Settings;
use particle_life::sim::Sim;
use rand::rngs::StdRng;
use rand::SeedableRng;

fn bench_settings(
    group: &mut BenchmarkGroup<WallTime>,
    name: &str,
    settings: Settings,
    wrap: bool,
) {
    let mut rng = StdRng::from_seed([5; 32]);
    let mut sim = Sim::new(settings, &mut rng);
    sim.wrap = wrap;

    group.bench_function(name, |b| b.iter(|| sim.step(1600.0, 900.0)));
}

fn bench_step(c: &mut Criterion) {
    let settings = [
        ("balanced", Settings::balanced()),
        ("chaos", Settings::chaos()),
        ("diversity", Settings::diversity()),
        ("frictionless", Settings::frictionless()),
        ("gliders", Settings::gliders()),
        ("homogeneity", Settings::homogeneity()),
        ("large_clusters", Settings::large_clusters()),
        ("medium_clusters", Settings::medium_clusters()),
        ("quiescence", Settings::quiescence()),
        ("small_clusters", Settings::small_clusters()),
    ];

    let mut non_wrapping = c.benchmark_group("non-wrapping");
    for (name, settings) in settings {
        bench_settings(&mut non_wrapping, name, settings, false);
    }
    non_wrapping.finish();

    let mut wrapping = c.benchmark_group("wrapping");
    for (name, settings) in settings {
        bench_settings(&mut wrapping, name, settings, true);
    }
    wrapping.finish();
}

criterion_group!(benches, bench_step);
criterion_main!(benches);
