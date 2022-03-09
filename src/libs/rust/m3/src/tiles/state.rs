/*
 * Copyright (C) 2020-2022 Nils Asmussen, Barkhausen Institut
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

use crate::col::Vec;
use crate::mem;
use crate::serialize::{copy_from_str, Source};

/// Serializes state into a vector.
pub struct StateSerializer<'v> {
    vec: &'v mut Vec<u64>,
}

impl<'v> StateSerializer<'v> {
    pub fn new(vec: &'v mut Vec<u64>) -> Self {
        Self { vec }
    }

    pub fn size(&self) -> usize {
        self.vec.len() * mem::size_of::<u64>()
    }

    pub fn words(&self) -> &[u64] {
        self.vec
    }

    pub fn push_word(&mut self, word: u64) {
        self.vec.push(word);
    }

    pub fn push_str(&mut self, b: &str) {
        let len = b.len() + 1;
        self.push_word(len as u64);

        let elems = (len + 7) / 8;
        let cur = self.vec.len();
        self.vec.resize(cur + elems, 0);

        unsafe {
            // safety: we know the pointer and length are valid
            copy_from_str(&mut self.vec.as_mut_slice()[cur..cur + elems], b);
        }
    }
}

/// Deserializes state from a slice
pub type StateDeserializer<'s> = Source<'s>;
