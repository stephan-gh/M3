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

use crate::int_enum;
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

int_enum! {
    /// The hash type ID for [`HashAlgorithm`].
    pub struct HashType : u64 {
        // Note: Must match the order in HashAlgorithm::ALL
        const SHA3_224 = 1;
        const SHA3_256 = 2;
        const SHA3_384 = 3;
        const SHA3_512 = 4;
        const SHAKE128 = 5;
        const SHAKE256 = 6;
    }
}

impl HashAlgorithm {
    pub const ALL: [&'static HashAlgorithm; 6] = [
        &Self::SHA3_224,
        &Self::SHA3_256,
        &Self::SHA3_384,
        &Self::SHA3_512,
        &Self::SHAKE128,
        &Self::SHAKE256,
    ];
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
        HashAlgorithm::ALL.get(ty.val as usize - 1).copied()
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
