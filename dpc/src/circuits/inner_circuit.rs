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

use crate::{execute_inner_circuit, record::Record, InnerPublicVariables, Parameters, PrivateKey};
use snarkvm_algorithms::{
    merkle_tree::MerklePath,
    traits::{CommitmentScheme, EncryptionScheme},
};
use snarkvm_r1cs::{errors::SynthesisError, ConstraintSynthesizer, ConstraintSystem};

#[derive(Derivative)]
#[derivative(Clone(bound = "C: Parameters"))]
pub struct InnerCircuit<C: Parameters> {
    public: InnerPublicVariables<C>,

    // Inputs for old records.
    old_records: Vec<Record<C>>,
    old_witnesses: Vec<MerklePath<C::RecordCommitmentTreeParameters>>,
    old_private_keys: Vec<PrivateKey<C>>,
    // Inputs for new records.
    new_records: Vec<Record<C>>,
    // Inputs for encryption of new records.
    new_records_encryption_randomness: Vec<<C::AccountEncryptionScheme as EncryptionScheme>::Randomness>,
    // Commitment to programs and local data.
    program_randomness: <C::ProgramCommitmentScheme as CommitmentScheme>::Randomness,
    local_data_commitment_randomizers: Vec<<C::LocalDataCommitmentScheme as CommitmentScheme>::Randomness>,
}

impl<C: Parameters> InnerCircuit<C> {
    pub fn blank() -> Self {
        // Construct the public variables.
        let public = InnerPublicVariables::blank();

        let old_records = vec![Record::default(); C::NUM_INPUT_RECORDS];
        let old_witnesses = vec![MerklePath::default(); C::NUM_INPUT_RECORDS];
        let old_private_keys = vec![PrivateKey::default(); C::NUM_INPUT_RECORDS];

        let new_records = vec![Record::default(); C::NUM_OUTPUT_RECORDS];
        let new_records_encryption_randomness =
            vec![<C::AccountEncryptionScheme as EncryptionScheme>::Randomness::default(); C::NUM_OUTPUT_RECORDS];

        let program_randomness = <C::ProgramCommitmentScheme as CommitmentScheme>::Randomness::default();
        let local_data_commitment_randomizers =
            vec![<C::LocalDataCommitmentScheme as CommitmentScheme>::Randomness::default(); C::NUM_TOTAL_RECORDS];

        Self {
            public,
            // Input records
            old_records,
            old_witnesses,
            old_private_keys,
            // Output records
            new_records,
            new_records_encryption_randomness,
            // Other stuff
            program_randomness,
            local_data_commitment_randomizers,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        public: InnerPublicVariables<C>,
        // Old records
        old_records: Vec<Record<C>>,
        old_witnesses: Vec<MerklePath<C::RecordCommitmentTreeParameters>>,
        old_private_keys: Vec<PrivateKey<C>>,
        // New records
        new_records: Vec<Record<C>>,
        new_records_encryption_randomness: Vec<<C::AccountEncryptionScheme as EncryptionScheme>::Randomness>,
        // Other stuff
        program_randomness: <C::ProgramCommitmentScheme as CommitmentScheme>::Randomness,
        local_data_commitment_randomizers: Vec<<C::LocalDataCommitmentScheme as CommitmentScheme>::Randomness>,
    ) -> Self {
        assert_eq!(C::NUM_INPUT_RECORDS, old_records.len());
        assert_eq!(C::NUM_INPUT_RECORDS, old_witnesses.len());
        assert_eq!(C::NUM_INPUT_RECORDS, old_private_keys.len());

        assert_eq!(C::NUM_OUTPUT_RECORDS, new_records.len());
        assert_eq!(C::NUM_OUTPUT_RECORDS, new_records_encryption_randomness.len());
        assert_eq!(C::NUM_OUTPUT_RECORDS, public.encrypted_record_hashes.len());

        Self {
            public,
            // Input records
            old_records,
            old_witnesses,
            old_private_keys,
            // Output records
            new_records,
            new_records_encryption_randomness,
            // Other stuff
            program_randomness,
            local_data_commitment_randomizers,
        }
    }
}

impl<C: Parameters> ConstraintSynthesizer<C::InnerScalarField> for InnerCircuit<C> {
    fn generate_constraints<CS: ConstraintSystem<C::InnerScalarField>>(
        &self,
        cs: &mut CS,
    ) -> Result<(), SynthesisError> {
        execute_inner_circuit::<C, CS>(
            cs,
            &self.public,
            // Old records
            &self.old_records,
            &self.old_witnesses,
            &self.old_private_keys,
            // New records
            &self.new_records,
            &self.new_records_encryption_randomness,
            // Other stuff
            &self.program_randomness,
            &self.local_data_commitment_randomizers,
        )?;
        Ok(())
    }
}
