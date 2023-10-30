/*
 * Copyright (C) 2021, Stephan Gerhold <stephan.gerhold@mailbox.tu-dresden.de>
 * This file is part of M3 (Microkernel-based SysteM for Heterogeneous Manycores).
 *
 * M3 is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License version 2 as
 * published by the Free Software Foundation.
 *
 * M3 is distributed in the hope that it will be useful, but
 * WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * General Public License version 2 for more details.
 */

use num_enum::IntoPrimitive;

use serde_repr::{Deserialize_repr, Serialize_repr};

use core::fmt;

/// A static definition of the properties of a hash algorithm.
pub struct HashAlgorithm {
    /// The name of the hash algorithm.
    pub name: &'static str,
    /// The type ID for the hash algorithm (used for IPC).
    pub ty: HashType,
    /// The block size in bytes (not bits).
    pub block_bytes: usize,
    /// The maximum allowed output size in bytes.
    /// Might be `usize::MAX` if the hash algorithm is a XOF (Extendable-output function)
    /// which can output an arbitrarily large number of output bytes.
    pub output_bytes: usize,
}

/// The hash type ID for [`HashAlgorithm`].
#[derive(Copy, Clone, Debug, Eq, PartialEq, IntoPrimitive, Serialize_repr, Deserialize_repr)]
#[repr(usize)]
pub enum HashType {
    // Note: Must match the order in HashAlgorithm::ALL
    SHA3_224 = 1,
    SHA3_256,
    SHA3_384,
    SHA3_512,
    SHAKE128,
    SHAKE256,
    CSHAKE128,
    CSHAKE256,
}

impl HashAlgorithm {
    pub const ALL: [&'static HashAlgorithm; 8] = [
        &Self::SHA3_224,
        &Self::SHA3_256,
        &Self::SHA3_384,
        &Self::SHA3_512,
        &Self::SHAKE128,
        &Self::SHAKE256,
        &Self::CSHAKE128,
        &Self::CSHAKE256,
    ];
    /// cSHAKE128 algorithm, with the assumption that the client will absorb the encoded
    /// function and customization string itself (e.g. using the extra cshake crate).
    /// WARNING: Will generate non-compliant output if this is not done.
    pub const CSHAKE128: HashAlgorithm = Self::new_shake("cshake128", HashType::CSHAKE128, 1344);
    /// cSHAKE256 algorithm, with the assumption that the client will absorb the encoded
    /// function and customization string itself (e.g. using the extra cshake crate).
    /// WARNING: Will generate non-compliant output if this is not done.
    pub const CSHAKE256: HashAlgorithm = Self::new_shake("cshake256", HashType::CSHAKE256, 1088);
    pub const MAX_OUTPUT_BYTES: usize = 512 / 8;
    pub const SHA3_224: HashAlgorithm = Self::new_sha3("sha3-224", HashType::SHA3_224, 1152, 224);
    pub const SHA3_256: HashAlgorithm = Self::new_sha3("sha3-256", HashType::SHA3_256, 1088, 256);
    pub const SHA3_384: HashAlgorithm = Self::new_sha3("sha3-384", HashType::SHA3_384, 832, 384);
    pub const SHA3_512: HashAlgorithm = Self::new_sha3("sha3-512", HashType::SHA3_512, 576, 512);
    pub const SHAKE128: HashAlgorithm = Self::new_shake("shake128", HashType::SHAKE128, 1344);
    pub const SHAKE256: HashAlgorithm = Self::new_shake("shake256", HashType::SHAKE256, 1088);

    const fn new_sha3(name: &'static str, ty: HashType, r: usize, output: usize) -> HashAlgorithm {
        HashAlgorithm {
            name,
            ty,
            block_bytes: r / 8,
            output_bytes: output / 8,
        }
    }

    const fn new_shake(name: &'static str, ty: HashType, r: usize) -> HashAlgorithm {
        HashAlgorithm {
            name,
            ty,
            block_bytes: r / 8,
            output_bytes: usize::MAX,
        }
    }

    /// Obtain the [`HashAlgorithm`] from the specified [`HashType`], if valid.
    pub fn from_type(ty: HashType) -> Option<&'static HashAlgorithm> {
        HashAlgorithm::ALL.get(ty as usize - 1).copied()
    }

    /// Obtain the [`HashAlgorithm`] from the specified name, if valid.
    pub fn from_name(name: &str) -> Option<&'static HashAlgorithm> {
        HashAlgorithm::ALL
            .iter()
            .copied()
            .find(|algo| algo.name == name)
    }

    /// Returns if this [`HashAlgorithm`] is an eXtendandable output function
    /// (XOF) that allows producing arbitrary amounts of output bytes.
    pub fn is_xof(&self) -> bool {
        self.output_bytes == usize::MAX
    }
}

impl fmt::Display for HashAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name)
    }
}
