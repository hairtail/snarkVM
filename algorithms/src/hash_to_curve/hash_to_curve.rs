// Copyright (C) 2019-2021 Aleo Systems Inc.
// This file is part of the snarkVM library.

// The snarkVM library is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The snarkVM library is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with the snarkVM library. If not, see <https://www.gnu.org/licenses/>.

use crate::crypto_hash::Blake2Xs;
use snarkvm_curves::AffineCurve;

/// Attempts to run hash-to-curve and returns the generator, message, and counter on sucess.
#[inline]
pub fn try_hash_to_curve<G: AffineCurve>(input: &str) -> Option<(G, String, usize)> {
    // Attempt to increment counter `k` at most `8 * G::SERIALIZED_SIZE` times.
    for k in 0..(8 * G::SERIALIZED_SIZE) {
        // Construct a new message.
        let message = format!("{} in {}", input, k);

        // Output the generator if a valid generator was found.
        if let Some(g) = hash_to_curve::<G>(&message) {
            return Some((g, message, k));
        }
    }
    None
}

/// Executes one round of hash-to-curve and returns a generator on success.
#[inline]
pub fn hash_to_curve<G: AffineCurve>(input: &str) -> Option<G> {
    debug_assert!(G::SERIALIZED_SIZE > 0);

    // Compute the digest for sampling the generator.
    let digest = Blake2Xs::evaluate(input.as_bytes(), G::SERIALIZED_SIZE as u16, "AleoHtC0".as_bytes());
    debug_assert!(digest.len() == G::SERIALIZED_SIZE);

    // Attempt to use the digest to derive a generator.
    G::from_random_bytes(&digest).and_then(|g| {
        debug_assert!(g.is_on_curve());

        let g = g.mul_by_cofactor();
        debug_assert!(g.is_on_curve());
        debug_assert!(g.is_in_correct_subgroup_assuming_on_curve());

        match !g.is_zero() {
            true => return Some(g),
            false => None,
        }
    })
}
