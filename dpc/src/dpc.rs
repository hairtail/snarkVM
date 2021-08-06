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

use crate::prelude::*;
use snarkvm_algorithms::{merkle_tree::MerklePath, prelude::*};
use snarkvm_fields::ToConstraintField;
use snarkvm_utilities::{has_duplicates, to_bytes_le, ToBytes, UniformRand};

use anyhow::Result;
use rand::{CryptoRng, Rng};

pub struct DPC<C: Parameters> {
    pub noop_program: NoopProgram<C>,
    pub inner_snark_parameters: (
        Option<<C::InnerSNARK as SNARK>::ProvingKey>,
        <C::InnerSNARK as SNARK>::VerifyingKey,
    ),
    pub outer_snark_parameters: (
        Option<<C::OuterSNARK as SNARK>::ProvingKey>,
        <C::OuterSNARK as SNARK>::VerifyingKey,
    ),
}

impl<C: Parameters> DPCScheme<C> for DPC<C> {
    type Account = Account<C>;
    type Authorization = TransactionAuthorization<C>;
    type Execution = Execution<C>;
    type Record = Record<C>;
    type Transaction = Transaction<C>;

    fn setup<R: Rng + CryptoRng>(rng: &mut R) -> Result<Self> {
        let setup_time = start_timer!(|| "DPC::setup");

        let noop_program_timer = start_timer!(|| "Noop program SNARK setup");
        let noop_program = NoopProgram::setup(rng)?;
        let noop_circuit = noop_program
            .find_circuit_by_index(0)
            .ok_or(DPCError::MissingNoopCircuit)?;
        let noop_program_execution = noop_program.execute_blank(noop_circuit.circuit_id())?;
        end_timer!(noop_program_timer);

        let snark_setup_time = start_timer!(|| "Execute inner SNARK setup");
        let inner_circuit = InnerCircuit::<C>::blank();
        let inner_snark_parameters = C::InnerSNARK::setup(&inner_circuit, &mut SRS::CircuitSpecific(rng))?;
        end_timer!(snark_setup_time);

        let snark_setup_time = start_timer!(|| "Execute outer SNARK setup");
        let inner_snark_vk: <C::InnerSNARK as SNARK>::VerifyingKey = inner_snark_parameters.1.clone().into();
        let inner_snark_proof = C::InnerSNARK::prove(&inner_snark_parameters.0, &inner_circuit, rng)?;

        let outer_snark_parameters = C::OuterSNARK::setup(
            &OuterCircuit::<C>::blank(inner_snark_vk, inner_snark_proof, noop_program_execution),
            &mut SRS::CircuitSpecific(rng),
        )?;

        end_timer!(snark_setup_time);
        end_timer!(setup_time);

        Ok(Self {
            noop_program,
            inner_snark_parameters: (Some(inner_snark_parameters.0), inner_snark_parameters.1),
            outer_snark_parameters: (Some(outer_snark_parameters.0), outer_snark_parameters.1),
        })
    }

    fn load(verify_only: bool) -> Result<Self> {
        let timer = start_timer!(|| "DPC::load");
        let noop_program = NoopProgram::load()?;
        let inner_snark_parameters = {
            let inner_snark_pk = C::inner_circuit_proving_key(!verify_only).clone();
            let inner_snark_vk = C::inner_circuit_verifying_key().clone();
            (inner_snark_pk, inner_snark_vk)
        };

        let outer_snark_parameters = {
            let outer_snark_pk = C::outer_circuit_proving_key(!verify_only).clone();
            let outer_snark_vk = C::outer_circuit_verifying_key().clone();
            (outer_snark_pk, outer_snark_vk)
        };
        end_timer!(timer);

        Ok(Self {
            noop_program,
            inner_snark_parameters,
            outer_snark_parameters,
        })
    }

    fn authorize<R: Rng + CryptoRng>(
        &self,
        private_keys: &Vec<<Self::Account as AccountScheme>::PrivateKey>,
        input_records: Vec<Self::Record>,
        output_records: Vec<Self::Record>,
        memo: Option<<Self::Transaction as TransactionScheme>::Memo>,
        rng: &mut R,
    ) -> Result<Self::Authorization> {
        assert_eq!(C::NUM_INPUT_RECORDS, private_keys.len());
        assert_eq!(C::NUM_INPUT_RECORDS, input_records.len());
        assert_eq!(C::NUM_OUTPUT_RECORDS, output_records.len());

        // Initialize the transaction kernel.
        let mut kernel = TransactionKernel {
            network_id: C::NETWORK_ID,
            serial_numbers: Vec::with_capacity(C::NUM_INPUT_RECORDS),
            commitments: Vec::with_capacity(C::NUM_OUTPUT_RECORDS),
            value_balance: AleoAmount::ZERO,
            memo: [0u8; 64],
        };

        // Initialize a vector for randomized private keys.
        let mut randomized_private_keys = Vec::with_capacity(C::NUM_INPUT_RECORDS);

        // Process the input records.
        for (i, record) in input_records.iter().enumerate().take(C::NUM_INPUT_RECORDS) {
            // Compute the serial numbers.
            let (serial_number, signature_randomizer) = record.to_serial_number(&private_keys[i])?;
            kernel.serial_numbers.push(serial_number);

            // Randomize the private key.
            randomized_private_keys.push(
                C::account_signature_scheme().randomize_private_key(&private_keys[i].sk_sig, &signature_randomizer)?,
            );

            if !record.is_dummy() {
                kernel.value_balance = kernel.value_balance.add(AleoAmount::from_bytes(record.value() as i64));
            }
        }

        // Process the output records.
        for record in output_records.iter().take(C::NUM_OUTPUT_RECORDS) {
            // Compute the commitments.
            kernel.commitments.push(record.commitment());

            if !record.is_dummy() {
                kernel.value_balance = kernel.value_balance.sub(AleoAmount::from_bytes(record.value() as i64));
            }
        }

        // Process the memo.
        match memo {
            Some(memo) => memo.write_le(&mut kernel.memo[..])?,
            None => (0..64)
                .map(|_| u8::rand(rng))
                .collect::<Vec<u8>>()
                .write_le(&mut kernel.memo[..])?,
        };

        // Construct the signature message.
        let signature_message = match kernel.is_valid() {
            true => kernel.to_signature_message()?,
            false => {
                return Err(DPCError::InvalidKernel(
                    kernel.network_id,
                    kernel.serial_numbers.len(),
                    kernel.commitments.len(),
                )
                .into());
            }
        };

        // Sign the transaction kernel to authorize the transaction.
        let mut signatures = Vec::with_capacity(C::NUM_INPUT_RECORDS);
        for i in 0..C::NUM_INPUT_RECORDS {
            signatures.push(C::account_signature_scheme().sign_randomized(
                &randomized_private_keys[i],
                &signature_message,
                rng,
            )?);
        }

        // Return the transaction authorization.
        Ok(TransactionAuthorization {
            kernel,
            input_records,
            output_records,
            signatures,
        })
    }

    fn execute<L: RecordCommitmentTree<C>, R: Rng + CryptoRng>(
        &self,
        private_keys: &Vec<<Self::Account as AccountScheme>::PrivateKey>,
        authorization: Self::Authorization,
        executables: Vec<Executable<C>>,
        ledger: &L,
        rng: &mut R,
    ) -> Result<Self::Transaction> {
        assert_eq!(C::NUM_INPUT_RECORDS, private_keys.len());
        assert_eq!(C::NUM_TOTAL_RECORDS, executables.len());

        let execution_timer = start_timer!(|| "DPC::execute");

        // Generate the local data.
        let local_data = authorization.to_local_data(rng)?;

        // Execute the programs.
        let mut executions = Vec::with_capacity(C::NUM_TOTAL_RECORDS);
        for (i, executable) in executables.iter().enumerate() {
            executions.push(executable.execute(i as u8, &local_data).unwrap());
        }

        // Compute the program commitment.
        let (program_commitment, program_randomness) = authorization.to_program_commitment(rng)?;

        // Compute the encrypted records.
        let (encrypted_records, encrypted_record_hashes, encrypted_record_randomizers) =
            authorization.to_encrypted_records(rng)?;

        let TransactionAuthorization {
            kernel,
            input_records,
            output_records,
            signatures,
        } = authorization;

        let TransactionKernel {
            network_id,
            serial_numbers,
            commitments,
            value_balance,
            memo,
        } = kernel.clone();

        // Construct the ledger witnesses.
        let ledger_digest = ledger.latest_digest()?;

        // Compute the ledger membership witnesses.
        let mut old_witnesses = Vec::with_capacity(C::NUM_INPUT_RECORDS);
        for record in input_records.iter().take(C::NUM_INPUT_RECORDS) {
            old_witnesses.push(match record.is_dummy() {
                true => MerklePath::default(),
                false => ledger.prove_cm(&record.commitment())?,
            });
        }

        // Construct the inner circuit public variables.
        let mut inner_public_variables = InnerPublicVariables {
            kernel: kernel.clone(),
            ledger_digest: ledger_digest.clone(),
            encrypted_record_hashes: encrypted_record_hashes.clone(),
            program_commitment: Some(program_commitment.clone()),
            local_data_root: Some(local_data.root().clone()),
        };

        // Compute the inner circuit proof.
        let inner_proof = {
            let circuit = InnerCircuit::<C>::new(
                inner_public_variables.clone(),
                input_records,
                old_witnesses,
                private_keys.clone(),
                output_records.clone(),
                encrypted_record_randomizers,
                program_randomness.clone(),
                local_data.leaf_randomizers().clone(),
            );

            let inner_snark_parameters = match &self.inner_snark_parameters.0 {
                Some(inner_snark_parameters) => inner_snark_parameters,
                None => return Err(DPCError::MissingInnerSnarkProvingParameters.into()),
            };

            C::InnerSNARK::prove(&inner_snark_parameters, &circuit, rng)?
        };

        // Verify that the inner circuit proof passes.
        assert!(C::InnerSNARK::verify(
            &self.inner_snark_parameters.1,
            &inner_public_variables,
            &inner_proof
        )?);

        let inner_snark_vk: <C::InnerSNARK as SNARK>::VerifyingKey = self.inner_snark_parameters.1.clone().into();
        let inner_circuit_id = C::inner_circuit_id();
        debug_assert_eq!(
            inner_circuit_id,
            &C::inner_circuit_id_crh().hash_field_elements(&inner_snark_vk.to_field_elements()?)?,
            "The DPC-loaded and Parameters-saved inner circuit IDs do not match"
        );

        let transaction_proof = {
            // These inner circuit public variables are allocated as private variables in the outer circuit,
            // as they are not included in the transaction broadcasted to the ledger.
            inner_public_variables.program_commitment = None;
            inner_public_variables.local_data_root = None;

            let circuit = OuterCircuit::<C>::new(
                inner_public_variables.clone(),
                inner_snark_vk,
                inner_proof,
                executions.to_vec(),
                program_commitment.clone(),
                program_randomness,
                local_data.root().clone(),
                inner_circuit_id.clone(),
            );

            let outer_proving_key = self
                .outer_snark_parameters
                .0
                .as_ref()
                .ok_or(DPCError::MissingOuterProvingKey)?;

            let outer_proof = C::OuterSNARK::prove(&outer_proving_key, &circuit, rng)?;

            // Verify the outer circuit proof passes.
            assert!(C::OuterSNARK::verify(
                &self.outer_snark_parameters.1,
                &OuterPublicVariables {
                    inner_public_variables,
                    inner_circuit_id: inner_circuit_id.clone(),
                },
                &outer_proof
            )?);

            outer_proof
        };
        end_timer!(execution_timer);

        Ok(Self::Transaction::new(
            Network::from_id(network_id),
            serial_numbers,
            commitments,
            value_balance,
            memo,
            ledger_digest,
            inner_circuit_id.clone(),
            transaction_proof,
            signatures,
            encrypted_records,
        ))
    }

    fn verify<L: RecordCommitmentTree<C> + RecordSerialNumberTree<C>>(
        &self,
        transaction: &Self::Transaction,
        ledger: &L,
    ) -> bool {
        let verify_time = start_timer!(|| "DPC::verify");

        // Returns false if the number of serial numbers in the transaction is incorrect.
        if transaction.serial_numbers().len() != C::NUM_INPUT_RECORDS {
            eprintln!("Transaction contains incorrect number of serial numbers");
            return false;
        }

        // Returns false if there are duplicate serial numbers in the transaction.
        if has_duplicates(transaction.serial_numbers().iter()) {
            eprintln!("Transaction contains duplicate serial numbers");
            return false;
        }

        // Returns false if the number of commitments in the transaction is incorrect.
        if transaction.commitments().len() != C::NUM_OUTPUT_RECORDS {
            eprintln!("Transaction contains incorrect number of commitments");
            return false;
        }

        // Returns false if there are duplicate commitments numbers in the transaction.
        if has_duplicates(transaction.commitments().iter()) {
            eprintln!("Transaction contains duplicate commitments");
            return false;
        }

        let ledger_time = start_timer!(|| "Ledger checks");

        // Returns false if any transaction serial number previously existed in the ledger.
        for sn in transaction.serial_numbers() {
            if ledger.contains_serial_number(sn) {
                eprintln!("Ledger already contains this transaction serial number.");
                return false;
            }
        }

        // Returns false if any transaction commitment previously existed in the ledger.
        for cm in transaction.commitments() {
            if ledger.contains_commitment(cm) {
                eprintln!("Ledger already contains this transaction commitment.");
                return false;
            }
        }

        // Returns false if the ledger digest in the transaction is invalid.
        if !ledger.is_valid_digest(&transaction.ledger_digest) {
            eprintln!("Ledger digest is invalid.");
            return false;
        }

        end_timer!(ledger_time);

        let signature_time = start_timer!(|| "Signature checks");

        // Returns false if the number of signatures in the transaction is incorrect.
        if transaction.signatures().len() != C::NUM_OUTPUT_RECORDS {
            eprintln!("Transaction contains incorrect number of commitments");
            return false;
        }

        let signature_message = match to_bytes_le![
            transaction.network_id(),
            transaction.serial_numbers(),
            transaction.commitments(),
            transaction.value_balance(),
            transaction.memo()
        ] {
            Ok(message) => message,
            _ => {
                eprintln!("Unable to construct signature message.");
                return false;
            }
        };

        for (pk, sig) in transaction.serial_numbers().iter().zip(transaction.signatures()) {
            match C::account_signature_scheme().verify(pk, &signature_message, sig) {
                Ok(is_valid) => {
                    if !is_valid {
                        eprintln!("Signature failed to verify.");
                        return false;
                    }
                }
                _ => {
                    eprintln!("Unable to verify signature.");
                    return false;
                }
            }
        }

        end_timer!(signature_time);

        // Construct the ciphertext hashes

        // Returns false if the number of encrypted records in the transaction is incorrect.
        if transaction.encrypted_records().len() != C::NUM_OUTPUT_RECORDS {
            eprintln!("Transaction contains incorrect number of encrypted records");
            return false;
        }

        let mut encrypted_record_hashes = Vec::with_capacity(C::NUM_OUTPUT_RECORDS);
        for encrypted_record in transaction.encrypted_records() {
            match encrypted_record.to_hash() {
                Ok(hash) => encrypted_record_hashes.push(hash),
                _ => {
                    eprintln!("Unable to hash encrypted record.");
                    return false;
                }
            }
        }

        let inner_snark_input = InnerPublicVariables {
            kernel: transaction.to_kernel(),
            ledger_digest: transaction.ledger_digest().clone(),
            encrypted_record_hashes,
            program_commitment: None,
            local_data_root: None,
        };

        debug_assert_eq!(
            C::inner_circuit_id(),
            &C::inner_circuit_id_crh()
                .hash_field_elements(&self.inner_snark_parameters.1.to_field_elements().unwrap())
                .unwrap(),
            "The DPC-loaded and Parameters-saved inner circuit IDs do not match"
        );

        let outer_public_variables = OuterPublicVariables {
            inner_public_variables: inner_snark_input,
            inner_circuit_id: C::inner_circuit_id().clone(),
        };

        match C::OuterSNARK::verify(
            &self.outer_snark_parameters.1,
            &outer_public_variables,
            &transaction.proof,
        ) {
            Ok(is_valid) => {
                if !is_valid {
                    eprintln!("Transaction proof failed to verify.");
                    return false;
                }
            }
            Err(error) => {
                eprintln!(
                    "Outer circuit verifier failed to validate transaction proof: {:?}",
                    error
                );
                return false;
            }
        }

        end_timer!(verify_time);

        true
    }

    /// Returns true iff all the transactions in the block are valid according to the ledger.
    fn verify_transactions<L: RecordCommitmentTree<C> + RecordSerialNumberTree<C>>(
        &self,
        transactions: &[Self::Transaction],
        ledger: &L,
    ) -> bool {
        for transaction in transactions {
            if !self.verify(transaction, ledger) {
                return false;
            }
        }

        true
    }
}
