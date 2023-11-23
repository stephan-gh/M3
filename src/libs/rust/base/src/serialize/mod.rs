/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
 *
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

//! Contains the serializing basics, which is used for IPC

mod de;
mod error;
mod ser;

pub use self::de::M3Deserializer;
pub use self::ser::{M3Serializer, Sink, SliceSink, VecSink};
pub use serde::{self, Deserialize, Deserializer, Serialize, Serializer};
pub use serde_bytes as bytes;

use crate::col::{String, Vec};
use crate::libc;

/// Constructs a message with the arguments `$args` into the given message buffer `$msg`
#[macro_export]
macro_rules! build_vmsg {
    ( $msg:expr, $( $args:expr ),* ) => ({
        // safety: we initialize these bytes below
        let sink = unsafe { $crate::serialize::SliceSink::new($msg.words_mut()) };
        let mut ser = $crate::serialize::M3Serializer::new(sink);
        $( ser.push(&$args); )*
        let bytes = ser.size();
        // safety: we just have initialized these bytes
        unsafe { $msg.set_size(bytes) };
    });
}

/// Copies the given string into the given word slice
///
/// # Safety
///
/// Assumes that words has sufficient space
pub unsafe fn copy_from_str(words: &mut [u64], s: &str) {
    let bytes = words.as_mut_ptr() as *mut u8;
    libc::memcpy(
        bytes as *mut libc::c_void,
        s.as_bytes().as_ptr() as *const libc::c_void,
        s.len(),
    );
    // null termination
    *bytes.add(s.len()) = 0u8;
}

/// Copies a string of given length from the given slice
///
/// # Safety
///
/// Assumes that `s` points to a valid string of given length
#[allow(clippy::uninit_vec)]
pub unsafe fn copy_str_from(s: &[u64], len: usize) -> String {
    let mut v = Vec::<u8>::with_capacity(len);
    // we deliberately use uninitialize memory here, because it's performance critical
    // safety: this is okay, because libc::memcpy (our implementation) does not read from `dst`
    v.set_len(len);
    let src = s.as_ptr() as *mut libc::c_void;
    let dst = v.as_mut_ptr() as *mut _ as *mut libc::c_void;
    libc::memcpy(dst, src, len);
    String::from_utf8(v).unwrap()
}

/// Returns a reference to the string in the given slice of given length
///
/// # Safety
///
/// Assumes that `s` points to a valid string of given length
pub unsafe fn str_slice_from(s: &[u64], len: usize) -> &'static str {
    let slice = core::slice::from_raw_parts(s.as_ptr() as *const u8, len);
    core::str::from_utf8(slice).unwrap()
}
