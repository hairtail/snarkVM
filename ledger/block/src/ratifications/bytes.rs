// Copyright (C) 2019-2023 Aleo Systems Inc.
// This file is part of the snarkVM library.

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at:
// http://www.apache.org/licenses/LICENSE-2.0

// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::*;

impl<N: Network> FromBytes for Ratifications<N> {
    /// Reads the ratifications from buffer.
    #[inline]
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the version.
        let version = u8::read_le(&mut reader)?;
        // Ensure the version is valid.
        if version != 1 {
            return Err(error("Invalid ratifications version"));
        }
        // Read the number of ratifications.
        let num_ratify: u32 = FromBytes::read_le(&mut reader)?;
        // Ensure the number of ratifications is within bounds.
        if num_ratify as usize > Self::MAX_RATIFICATIONS {
            return Err(error("Failed to read ratifications: too many ratifications"));
        }
        // Read the ratifications.
        let ratifications = (0..num_ratify).map(|_| FromBytes::read_le(&mut reader)).collect::<Result<Vec<_>, _>>()?;
        // Return the ratifications.
        Self::try_from(ratifications).map_err(error)
    }
}

impl<N: Network> ToBytes for Ratifications<N> {
    /// Writes the ratifications to a buffer.
    #[inline]
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        // Write the version.
        1u8.write_le(&mut writer)?;
        // Write the number of ratifications.
        u32::try_from(self.ratifications.len()).map_err(|e| error(e.to_string()))?.write_le(&mut writer)?;
        // Write the ratifications.
        self.ratifications.values().try_for_each(|ratification| ratification.write_le(&mut writer))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console::network::Testnet3;

    type CurrentNetwork = Testnet3;

    const ITERATIONS: u32 = 100;

    #[test]
    fn test_bytes() -> Result<()> {
        let rng = &mut TestRng::default();

        for _ in 0..ITERATIONS {
            let expected = crate::ratifications::test_helpers::sample_block_ratifications(rng);
            // Check the byte representation.
            let expected_bytes = expected.to_bytes_le()?;
            assert_eq!(expected, Ratifications::read_le(&expected_bytes[..])?);
            assert!(Ratifications::<CurrentNetwork>::read_le(&expected_bytes[1..]).is_err());
        }
        Ok(())
    }
}
