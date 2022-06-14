use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::thread_rng;
use small_powers_of_tau::{
    accumulator::Accumulator, keypair::PrivateKey, serialisation::SubgroupCheck,
};

fn update_algo() {
    use small_powers_of_tau::accumulator::*;

    let params = Parameters {
        num_g1_elements_needed: 2usize.pow(16),
        num_g2_elements_needed: 2,
    };

    let num_coefficients: usize = 2usize.pow(16);

    // Simulate deserialisation
    let acc = Accumulator::new_for_kzg(num_coefficients);
    let bytes = acc.serialise();
    let mut acc = Accumulator::deserialise(&bytes, params, SubgroupCheck::Partial);

    let mut rng = &mut thread_rng();
    let priv_key = PrivateKey::rand(rng);
    acc.update(priv_key);
    let bytes = acc.serialise();
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("update algo", |b| b.iter(|| black_box(update_algo())));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
