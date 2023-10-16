/*
 * Copyright (C) 2023-2024, Stephan Gerhold <stephan@gerhold.net>
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

use base::crypto::{HashAlgorithm, HashType};
use base::io::LogFlags;
use base::log;
use cshake::kmac;

use crate::{Secret, KECACC};

// KMAC256
const KMAC_HASH_TYPE: HashType = HashType::CSHAKE256;
const KMAC_HASH_ALGO: &HashAlgorithm = &HashAlgorithm::CSHAKE256;
const KMAC_BLOCK_BYTES: usize = KMAC_HASH_ALGO.block_bytes;
const KMAC_CUSTOM_CDI: &str = "DICE";
const KMAC_KEY_PAD_CDI: &str = "CDI";
const CDI_BITS: usize = 512;
const CDI_BYTES: usize = CDI_BITS / u8::BITS as usize;

/// A full KMAC input block that contains an opaque secret key together with
/// the KMAC headers. Can be used to derive additional secrets using KMAC.
///
/// The secret key is contained in the buffer but its exact location and
/// length is intentionally undefined. The secret key should be *never* used
/// directly.
pub type OpaqueKMacKey = [u8; KMAC_BLOCK_BYTES];

pub fn derive_key(key: &Secret<OpaqueKMacKey>, s: &str, data: &[u8], out: &mut [u8]) {
    KECACC.start_init(KMAC_HASH_TYPE);
    let mut buf = [0; KMAC_BLOCK_BYTES];
    let size = kmac::prepend_header(&mut buf[..], s, KMAC_BLOCK_BYTES);
    KECACC.start_absorb(&buf[..size]);
    KECACC.start_absorb(&key.secret[..]);
    KECACC.start_absorb(data);
    let size = kmac::append_output_length(&mut buf[..], out.len() * u8::BITS as usize);
    KECACC.start_absorb_last(&buf[..size]);
    KECACC.start_squeeze(out);
    KECACC.start_init(HashType::NONE); // Reset state
    KECACC.poll_complete_barrier();

    log!(
        LogFlags::RoTDbg,
        "Derived {} key: {:?}",
        s,
        Secret::new(out)
    );
}

pub fn derive_cdi(key: &Secret<OpaqueKMacKey>, data: &[u8], out: &mut Secret<OpaqueKMacKey>) {
    let off = kmac::write_partial_key(&mut out.secret, KMAC_KEY_PAD_CDI.as_bytes(), CDI_BYTES);
    derive_key(
        key,
        KMAC_CUSTOM_CDI,
        data,
        &mut out.secret[off..off + CDI_BYTES],
    );
}
