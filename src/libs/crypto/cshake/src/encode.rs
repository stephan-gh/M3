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

//! Implementations of the encoding functions from NIST SP 800-185.

/// From NIST SP 800-185, section 2.3.1 "Integer to Byte String Encoding":
/// left_encode(x) encodes the integer x as a byte string in a way that can be unambiguously parsed
/// from the beginning of the string by inserting the length of the byte string before the byte string
/// representation of x.
///
/// In this implementation, the encoding is written to the start of the buffer, and the number of
/// bytes written is returned.
///
/// Up to `size_of::<usize>() + 1` bytes are written, depending on the value of x.
pub fn left_encode(buf: &mut [u8], x: usize) -> usize {
    let be = x.to_be_bytes();
    let pos = be.iter().position(|&b| b != 0).unwrap_or(be.len() - 1);
    let size = be.len() - pos;
    buf[0] = size as u8;
    buf[1..=size].copy_from_slice(&be[pos..]);
    size + 1
}

/// From NIST SP 800-185, section 2.3.1 "Integer to Byte String Encoding":
/// right_encode(x) encodes the integer x as a byte string in a way that can be unambiguously parsed
/// from the end of the string by inserting the length of the byte string after the byte string
/// representation of x.
///
/// In this implementation, the encoding is written to the start of the buffer, and the number of
/// bytes written is returned.
///
/// Up to `size_of::<usize>() + 1` bytes are written, depending on the value of x.
pub fn right_encode(buf: &mut [u8], x: usize) -> usize {
    let be = x.to_be_bytes();
    let pos = be.iter().position(|&b| b != 0).unwrap_or(be.len() - 1);
    let size = be.len() - pos;
    buf[..size].copy_from_slice(&be[pos..]);
    buf[size] = size as u8;
    size + 1
}

/// From NIST SP 800-185, section 2.3.2 "String Encoding":
/// The encode_string function is used to encode bit strings in a way that may be parsed
/// unambiguously from the beginning of the string, S.
///
/// In this implementation, the encoding is written to the start of the buffer, and the number of
/// bytes written is returned.
///
/// Up to `size_of::<usize>() + 1 + s.len()` bytes are written, depending on the length of s.
pub fn encode_string(buf: &mut [u8], s: &[u8]) -> usize {
    let len = s.len();
    let len_size = left_encode(buf, len * u8::BITS as usize);
    buf[len_size..len_size + len].copy_from_slice(s);
    len_size + len
}

/// From NIST SP 800-185, section 2.3.3 "Padding":
/// The bytepad(X, w) function prepends an encoding of the integer w to an input string X, then pads
/// the result with zeros until it is a byte string whose length in bytes is a multiple of w. In general,
/// bytepad is intended to be used on encoded strings—the byte string bytepad(encode_string(S), w)
/// can be parsed unambiguously from its beginning, whereas bytepad does not provide
/// unambiguous padding for all input strings.
///
/// In this implementation, the `write()` function is called with the remaining space in the buffer.
/// The total size is determined and padded up to a multiple of `w` bytes. Returns the number of
/// bytes written.
pub fn bytepad(buf: &mut [u8], w: usize, write: impl FnOnce(&mut [u8]) -> usize) -> usize {
    let mut off = left_encode(buf, w);
    off += write(&mut buf[off..]);
    let w_len = off + w - (off % w);
    buf[off..w_len].fill(0);
    w_len
}
