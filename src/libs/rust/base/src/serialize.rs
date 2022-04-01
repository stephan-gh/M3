/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2021 Nils Asmussen, Barkhausen Institut
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

use crate::col::{String, Vec};
use crate::errors::{Code, Error};
use crate::libc;
use crate::mem;

/// For types that can be marshalled into a [`Sink`].
pub trait Marshallable {
    /// Writes this object into the given sink
    fn marshall(&self, s: &mut Sink<'_>);
}

/// For types that can be unmarshalled from a [`Source`].
pub trait Unmarshallable: Sized {
    /// Reads an object from the given source and returns it
    fn unmarshall(s: &mut Source<'_>) -> Result<Self, Error>;
}

/// A sink for marshalling into a slice
pub struct Sink<'s> {
    arr: &'s mut [u64],
    pos: usize,
}

impl<'s> Sink<'s> {
    pub fn new(slice: &'s mut [u64]) -> Self {
        Self { arr: slice, pos: 0 }
    }

    #[inline(always)]
    pub fn size(&self) -> usize {
        self.pos * mem::size_of::<u64>()
    }

    #[inline(always)]
    pub fn words(&self) -> &[u64] {
        &self.arr[0..self.pos]
    }

    #[inline(always)]
    pub fn push(&mut self, item: &dyn Marshallable) {
        item.marshall(self);
    }

    #[inline(always)]
    pub fn push_word(&mut self, word: u64) {
        self.arr[self.pos] = word;
        self.pos += 1;
    }

    pub fn push_str(&mut self, b: &str) {
        let len = b.len() + 1;
        self.push_word(len as u64);

        // safety: we know the pointer and length are valid
        unsafe { copy_from_str(&mut self.arr[self.pos..], b) };
        self.pos += (len + 7) / 8;
    }
}

/// A source for unmarshalling that uses a slice internally.
#[derive(Debug)]
pub struct Source<'s> {
    slice: &'s [u64],
    pos: usize,
}

impl<'s> Source<'s> {
    /// Creates a new `Source` for given slice.
    pub fn new(s: &'s [u64]) -> Source<'s> {
        Source { slice: s, pos: 0 }
    }

    pub fn size(&self) -> usize {
        self.slice.len()
    }

    /// Pops an object of type `T` from the source.
    pub fn pop<T: Unmarshallable>(&mut self) -> Result<T, Error> {
        T::unmarshall(self)
    }

    pub fn pop_word(&mut self) -> Result<u64, Error> {
        if self.pos >= self.slice.len() {
            return Err(Error::new(Code::InvArgs));
        }

        self.pos += 1;
        Ok(self.slice[self.pos - 1])
    }

    pub fn pop_str(&mut self) -> Result<String, Error> {
        // safety: we know that the pointer and length are okay
        self.do_pop_str(|slice, pos, len| unsafe { copy_str_from(&slice[pos..], len - 1) })
    }

    pub fn pop_str_slice(&mut self) -> Result<&'static str, Error> {
        // safety: we know that the pointer and length are okay
        self.do_pop_str(|slice, pos, len| unsafe { str_slice_from(&slice[pos..], len - 1) })
    }

    fn do_pop_str<T, F>(&mut self, f: F) -> Result<T, Error>
    where
        F: Fn(&'s [u64], usize, usize) -> T,
    {
        let len = self.pop_word()? as usize;

        let npos = self.pos + (len + 7) / 8;
        if len == 0 || npos > self.slice.len() {
            return Err(Error::new(Code::InvArgs));
        }

        let res = f(self.slice, self.pos, len);
        self.pos = npos;
        Ok(res)
    }
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

macro_rules! impl_xfer_prim {
    ( $t:ty ) => {
        impl Marshallable for $t {
            #[inline(always)]
            fn marshall(&self, s: &mut Sink<'_>) {
                s.push_word(*self as u64);
            }
        }
        impl Unmarshallable for $t {
            #[inline(always)]
            fn unmarshall(s: &mut Source<'_>) -> Result<Self, Error> {
                s.pop_word().map(|v| v as $t)
            }
        }
    };
}

impl_xfer_prim!(u8);
impl_xfer_prim!(i8);
impl_xfer_prim!(u16);
impl_xfer_prim!(i16);
impl_xfer_prim!(u32);
impl_xfer_prim!(i32);
impl_xfer_prim!(u64);
impl_xfer_prim!(i64);
impl_xfer_prim!(usize);
impl_xfer_prim!(isize);

impl Marshallable for bool {
    #[inline(always)]
    fn marshall(&self, s: &mut Sink<'_>) {
        s.push_word(*self as u64);
    }
}
impl Unmarshallable for bool {
    #[inline(always)]
    fn unmarshall(s: &mut Source<'_>) -> Result<Self, Error> {
        s.pop_word().map(|v| v == 1)
    }
}

impl<'a> Marshallable for &'a str {
    fn marshall(&self, s: &mut Sink<'_>) {
        s.push_str(self);
    }
}
impl Unmarshallable for &'static str {
    fn unmarshall(s: &mut Source<'_>) -> Result<Self, Error> {
        s.pop_str_slice()
    }
}

impl Marshallable for String {
    fn marshall(&self, s: &mut Sink<'_>) {
        s.push_str(self.as_str());
    }
}
impl Unmarshallable for String {
    fn unmarshall(s: &mut Source<'_>) -> Result<Self, Error> {
        s.pop_str()
    }
}
