/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
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
use crate::serialize::{copy_from_str, Source};
use crate::util;

/// Serializes state into a vector.
pub struct StateSerializer {
    vec: Vec<u64>,
}

impl Default for StateSerializer {
    fn default() -> Self {
        StateSerializer { vec: Vec::new() }
    }
}

impl StateSerializer {
    pub fn size(&self) -> usize {
        self.vec.len() * util::size_of::<u64>()
    }

    pub fn words(&self) -> &[u64] {
        &self.vec
    }

    pub fn push_word(&mut self, word: u64) {
        self.vec.push(word);
    }

    pub fn push_str(&mut self, b: &str) {
        let len = b.len() + 1;
        self.push_word(len as u64);

        let elems = (len + 7) / 8;
        let cur = self.vec.len();
        self.vec.reserve_exact(elems);

        unsafe {
            // safety: will be initialized below
            self.vec.set_len(cur + elems);
            // safety: we know the pointer and length are valid
            copy_from_str(&mut self.vec.as_mut_slice()[cur..cur + elems], b);
        }
    }
}

/// Deserializes state from a slice
pub type StateDeserializer<'s> = Source<'s>;
