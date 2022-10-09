// An update proof shows two things:
// - One knows the discrete log to a secret `p` via KoE
// - `p` was used to update an existing point A to a new point A'

use crate::shared_secret::SharedSecretChain;
use ark_bls12_381::{G1Projective, G2Projective};
use crate::interop_point_encoding::serialize_g2;
use ark_ec::ProjectiveCurve;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpdateProof {
    // A commitment to the secret scalar `p`
    pub(crate) commitment_to_secret: G2Projective,
    // This is the degree-1 element of the SRS after it has been
    // updated by the contributor
    pub(crate) new_accumulated_point: G1Projective,
}

impl UpdateProof {
    // Verifies a list of update of update proofs using `SharedSecretChain` as a subroutine
    pub(crate) fn verify_chain(
        starting_point: G1Projective,
        update_proofs: &[UpdateProof],
    ) -> bool {
        let mut chain = SharedSecretChain::starting_from(starting_point);

        for update_proof in update_proofs {
            // Add the new accumulated point into the chain along with a witness that attests to the
            // transition from the previous point to it.
            chain.extend(
                update_proof.new_accumulated_point,
                update_proof.commitment_to_secret,
            );
        }

        chain.verify()
    }
    // Returns commitment_to_secret (g2)
    pub fn get_commitment_to_secret(&self) -> String {
        let mut commitment = hex::encode(serialize_g2(&self.commitment_to_secret.into_affine()));
        commitment.insert_str(0, "0x");
        commitment
    }
}
