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

//! Helper functions for generating cSHAKE hashes as defined in NIST SP 800-185.

use crate::encode::*;

/// Writes the cSHAKE header (function name and customization string) to the start of the buffer.
/// This should be absorbed before (or prepended to) the actual input data to produce valid cSHAKE
/// hashes. Block bytes is the block size of the underlying hash function (cSHAKE128 or cSHAKE256).
pub fn prepend_header(buf: &mut [u8], n: &str, s: &str, block_bytes: usize) -> usize {
    bytepad(buf, block_bytes, |buf| {
        let mut pos = 0;
        pos += encode_string(&mut buf[pos..], n.as_bytes());
        pos += encode_string(&mut buf[pos..], s.as_bytes());
        pos
    })
}
