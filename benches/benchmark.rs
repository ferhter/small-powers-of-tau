use ark_bls12_381::{G1Projective, G2Projective};
use ark_ec::ProjectiveCurve;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::thread_rng;
use small_powers_of_tau::{keypair::PrivateKey, serialisation::SubgroupCheck, srs::SRS};

fn update_algo() {
    use small_powers_of_tau::srs::*;

    let params = Parameters {
        num_g1_elements_needed: 2usize.pow(16),
        num_g2_elements_needed: 2,
    };

    // Simulate deserialisation
    let acc = SRS::new(params);
    let bytes = acc.serialise();
    let mut acc = SRS::deserialise(&bytes, params);

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
