/*
 * Copyright (C) 2020-2021 Nils Asmussen, Barkhausen Institut
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
use crate::vec;

/// An array of bits
///
/// Besides storing a sequence of bits, `BitArray` keeps track of the first clear bit in the
/// sequence to provide quick access to this information.
pub struct BitArray {
    bits: usize,
    first_clear: usize,
    words: Vec<usize>,
}

fn word_bits() -> usize {
    mem::size_of::<usize>() * 8
}

fn idx(bit: usize) -> usize {
    bit / word_bits()
}

fn bitpos(bit: usize) -> usize {
    1 << (bit % word_bits())
}

impl BitArray {
    /// Creates a new `BitArray` with the given number of bits
    pub fn new(bits: usize) -> Self {
        let word_count = (bits + word_bits() - 1) / word_bits();
        BitArray {
            bits,
            words: vec![0; word_count],
            first_clear: 0,
        }
    }

    /// Returns the number of bits in the array
    pub fn size(&self) -> usize {
        self.bits
    }

    /// Returns true if the bit with given index is set
    pub fn is_set(&self, bit: usize) -> bool {
        self.words[idx(bit)] & bitpos(bit) != 0
    }

    /// Returns the index of the first bit that is not set
    pub fn first_clear(&self) -> usize {
        self.first_clear
    }

    /// Sets the bit with given index
    pub fn set(&mut self, bit: usize) {
        self.words[idx(bit)] |= bitpos(bit);
        if bit == self.first_clear {
            self.first_clear += 1;
            while self.first_clear < self.bits && self.is_set(self.first_clear) {
                self.first_clear += 1;
            }
        }
    }

    /// Clears the bit with given index
    pub fn clear(&mut self, bit: usize) {
        self.words[idx(bit)] &= !bitpos(bit);
        if bit < self.first_clear {
            self.first_clear = bit;
        }
    }
}
