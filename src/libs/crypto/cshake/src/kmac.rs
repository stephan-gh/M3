/*
 * Copyright (C) 2023, Stephan Gerhold <stephan.gerhold@mailbox.tu-dresden.de>
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

//! Helper functions for generating KMAC hashes as defined in NIST SP 800-185.

use crate::cshake;
use crate::encode::*;

/// The cSHAKE function name for KMAC as defined in NIST SP 800-185.
pub const FUNCTION_NAME: &str = "KMAC";

/// The output length to append for KMACXOF128 and KMACXOF256 as defined in NIST SP 800-185.
pub const XOF_OUTPUT_LENGTH: usize = 0;

/// Writes the KMAC header (function name and customization string) to the start of the buffer.
/// This should be absorbed **before** (or prepended to) the key and actual input data to produce
/// valid KMAC hashes. Block bytes is the block size of the underlying hash function (cSHAKE128 for
/// KMAC128 or cSHAKE256 for KMAC256).
pub fn prepend_header(buf: &mut [u8], s: &str, block_bytes: usize) -> usize {
    cshake::prepend_header(buf, FUNCTION_NAME, s, block_bytes)
}

/// Writes the KMAC key to the start of the buffer. This should be absorbed **before**
/// (or prepended to) the actual input data to produce valid KMAC hashes. Block bytes is the block
/// size of the underlying hash function (cSHAKE128 for KMAC128 or cSHAKE256 for KMAC256).
pub fn prepend_key(buf: &mut [u8], key: &[u8], block_bytes: usize) -> usize {
    bytepad(buf, block_bytes, |buf| encode_string(buf, key))
}

/// Writes the KMAC output length (in bits) to the start of the buffer. This should be absorbed
/// **after** (or appended to) the actual input data to produce valid KMAC hashes.
pub fn append_output_length(buf: &mut [u8], bits: usize) -> usize {
    right_encode(buf, bits)
}
