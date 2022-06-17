
#[macro_use]
use ark_bls12_381::{Fr, G1Affine, G1Projective, G2Affine, G2Projective};
use ark_ec::{AffineCurve, PairingEngine, ProjectiveCurve};
use ark_ff::{Field, PrimeField, Zero};
use std::fs::File;
use std::io::Write;

use crate::{keypair::PrivateKey, update_proof::UpdateProof, serialisation::SubgroupCheck};

use rand::thread_rng;

use js_sys;
use web_sys;
use console_error_panic_hook;
use wasm_bindgen::prelude::*;
pub use wasm_bindgen_rayon::init_thread_pool;

#[derive(Debug, Clone)]
pub struct Accumulator {
    pub(crate) tau_g1: Vec<G1Projective>,
    pub(crate) tau_g2: Vec<G2Projective>,
}
#[derive(Debug, Clone, Copy)]
pub struct Parameters {
    pub num_g1_elements_needed: usize,
    pub num_g2_elements_needed: usize,
}

macro_rules! log {
    ($($t:tt)*) => (web_sys::console::log_1(&format_args!($($t)*).to_string().into()))
}

impl Accumulator {

    // Creates a powers of tau ceremony.
    // This is not compatible with the BGM17 Groth16 powers of tau ceremony (notice there is no \alpha, \beta)
    pub fn new(parameters: Parameters) -> Accumulator {
        Self {
            tau_g1: vec![
                G1Projective::prime_subgroup_generator();
                parameters.num_g1_elements_needed
            ],
            tau_g2: vec![
                G2Projective::prime_subgroup_generator();
                parameters.num_g2_elements_needed
            ],
        }
    }

    // Creates a ceremony for the kzg polynomial commitment scheme
    // One should input the number of coefficients for the polynomial with the
    // highest degree that you wish to use kzg with.
    //
    // Example; a degree 2 polynomial has 3 coefficients ax^0 + bx^1 + cx^2
    pub fn new_for_kzg(num_coefficients: usize) -> Accumulator {
        // The amount of G2 elements needed for KZG based commitment schemes
        const NUM_G2_ELEMENTS_NEEDED: usize = 2;

        let params = Parameters {
            num_g1_elements_needed: num_coefficients,
            num_g2_elements_needed: NUM_G2_ELEMENTS_NEEDED,
        };

        Accumulator::new(params)
    }

    // Updates the accumulator and produces a proof of this update
    pub fn update(&mut self, private_key: PrivateKey) -> UpdateProof {
        // Save the previous s*G_1 element, then update the accumulator and save the new s*private_key*G_1 element
        let previous_tau = self.tau_g1[1];
        self.update_accumulator(private_key.tau);
        let updated_tau = self.tau_g1[1];

        UpdateProof {
            commitment_to_secret: private_key.to_public(),
            previous_accumulated_point: previous_tau,
            new_accumulated_point: updated_tau,
        }
    }

    // Inefficiently, updates the group elements using a users private key
    fn update_accumulator(&mut self, private_key: Fr) {
        use ark_ec::wnaf::WnafContext;
        use rayon::prelude::*;

        let max_number_elements = std::cmp::max(self.tau_g1.len(), self.tau_g2.len());

        let powers_of_priv_key = vandemonde_challenge(private_key, max_number_elements);

        let wnaf = WnafContext::new(3);

        self.tau_g1
            .par_iter_mut()
            .skip(1)
            .zip(&powers_of_priv_key)
            .for_each(|(tg1, priv_pow)| {
                *tg1 = wnaf.mul(*tg1, priv_pow);
            });

        self.tau_g2
            .par_iter_mut()
            .skip(1)
            .zip(&powers_of_priv_key)
            .for_each(|(tg2, priv_pow)| {
                *tg2 = wnaf.mul(*tg2, priv_pow);
            })
    }

    // Verify whether the transition from one SRS to the other was valid
    //
    // Most of the time, there will be a single update proof for verifying that a contribution did indeed update the SRS correctly.
    //
    // After the ceremony is over, one may use this method to check all update proofs that were given in the ceremony
    pub fn verify_updates(
        // TODO: We do not _need_ the whole `before` accumulator, this API is just a bit cleaner
        before: &Accumulator,
        after: &Accumulator,
        update_proofs: &[UpdateProof],
    ) -> bool {
        let first_update = update_proofs.first().expect("expected at least one update");
        let last_update = update_proofs.last().expect("expected at least one update");

        // 1a. Check that the updates started from the starting SRS
        if before.tau_g1[1] != first_update.previous_accumulated_point {
            return false;
        }
        // 1b.Check that the updates finished at the ending SRS
        if after.tau_g1[1] != last_update.new_accumulated_point {
            return false;
        }

        // 2. Check the update proofs are correct and form a chain of updates
        if !UpdateProof::verify_chain(update_proofs) {
            return false;
        }

        // 3. Check that the degree-0 component is not the identity element
        // No need to check the other elements because the structure check will fail
        // if they are also not the identity element
        if after.tau_g1[0].is_zero() {
            return false;
        }
        if after.tau_g2[0].is_zero() {
            return false;
        }

        // 3. Check that the new SRS goes up in incremental powers
        if !after.structure_check() {
            return false;
        }

        true
    }

    pub fn verify_update(
        before: &Accumulator,
        after: &Accumulator,
        update_proof: &UpdateProof,
    ) -> bool {
        Accumulator::verify_updates(before, after, &[*update_proof])
    }

    // Inefficiently checks that the srs has the correct structure
    // Meaning each subsequent element is increasing the index of tau for both G_1 and G_2 elements
    fn structure_check(&self) -> bool {
        let tau_g2_0 = self.tau_g2[0];
        let tau_g2_1 = self.tau_g2[1];

        let tau_g1_0 = self.tau_g1[0];
        let tau_g1_1 = self.tau_g1[1];

        // Check G_1 elements
        let power_pairs = self.tau_g1.as_slice().windows(2);
        for pair in power_pairs {
            let tau_i = pair[0]; // tau^i
            let tau_i_next = pair[1]; // tau^{i+1}
            let p1 = ark_bls12_381::Bls12_381::pairing(tau_i_next, tau_g2_0);
            let p2 = ark_bls12_381::Bls12_381::pairing(tau_i, tau_g2_1);
            if p1 != p2 {
                return false;
            }
        }

        // Check G_2 elements
        let power_pairs = self.tau_g2.as_slice().windows(2);
        for pair in power_pairs {
            let tau_i = pair[0]; // tau^i
            let tau_i_next = pair[1]; // tau^{i+1}
            let p1 = ark_bls12_381::Bls12_381::pairing(tau_g1_0, tau_i_next);
            let p2 = ark_bls12_381::Bls12_381::pairing(tau_g1_1, tau_i);
            if p1 != p2 {
                return false;
            }
        }

        true
    }
}

fn vandemonde_challenge(x: Fr, n: usize) -> Vec<Fr> {
    let mut challenges: Vec<Fr> = Vec::with_capacity(n);
    challenges.push(x);
    for i in 0..n - 1 {
        challenges.push(challenges[i] * x);
    }
    challenges
}

// Javascript entry point
#[wasm_bindgen]
pub fn contribute(points: Vec<u8>, g1_size: usize, g2_size: usize) -> Result<Vec<u8>, JsValue> {
    console_error_panic_hook::set_once();
    log!("Init contribute");


    // Put the JS parmeters into an Accumulator
    let params = Parameters {
            num_g1_elements_needed: g1_size,
            num_g2_elements_needed: g2_size,
    };

    log!("g1_size: {:?}", g1_size);
    log!("g2_size: {:?}", g2_size);

    let mut accumulator: Accumulator = Accumulator::deserialise(&points, params, SubgroupCheck::Partial);

    log!("get entropy");

    let mut rng = &mut thread_rng();
    let priv_key = PrivateKey::rand(rng);

    log!("update");
    let update_proof: UpdateProof = accumulator.update(priv_key);

    log!("serialise");
    let mut update_bytes: Vec<u8> = accumulator.serialise();

    Ok(update_bytes)
}

#[test]
fn reject_private_key_one() {
    // This test ensures that one cannot update the SRS using either 0 or 1

    let before = Accumulator::new_for_kzg(100);
    let mut after = before.clone();

    let secret = PrivateKey::from_u64(1);
    let update_proof = after.update(secret);

    assert!(!Accumulator::verify_update(&before, &after, &update_proof));
}
#[test]
fn reject_private_key_zero() {
    // This test ensures that one cannot update the SRS using either 0 or 1

    let before = Accumulator::new_for_kzg(100);
    let mut after = before.clone();

    let secret = PrivateKey::from_u64(0);
    let update_proof = after.update(secret);

    assert!(!Accumulator::verify_update(&before, &after, &update_proof));
}

#[test]
fn acc_fuzz() {
    let secret_a = PrivateKey::from_u64(252);
    let secret_b = PrivateKey::from_u64(512);
    let secret_c = PrivateKey::from_u64(789);

    let mut acc = Accumulator::new_for_kzg(100);

    // Simulate 3 participants updating the accumulator, one after the other
    let update_proof_1 = acc.update(secret_a);
    let update_proof_2 = acc.update(secret_b);
    let update_proof_3 = acc.update(secret_c);

    // This verifies each update proof makes the correct transition, but it does not link
    // the update proofs, so these could in theory be updates to different accumulators
    assert!(update_proof_1.verify());
    assert!(update_proof_2.verify());
    assert!(update_proof_3.verify());

    // Here we also verify the chain, if elements in the vector are out of place, the proof will also fail
    assert!(UpdateProof::verify_chain(&[
        update_proof_1,
        update_proof_2,
        update_proof_3,
    ]));
}

#[test]
fn write_new() {

    let num_coefficients: usize = 2usize.pow(16);
    let acc = Accumulator::new_for_kzg(num_coefficients);
    let bytes = acc.serialise();

    let mut f = File::create("new_kzg.pot").unwrap();
    f.write(&bytes).expect("unable to write params");
}
