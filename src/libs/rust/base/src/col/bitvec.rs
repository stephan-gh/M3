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

use col::Vec;
use util;

pub struct BitVec {
    bits: usize,
    first_clear: usize,
    words: Vec<usize>,
}

fn word_bits() -> usize {
    util::size_of::<usize>() * 8
}

fn idx(bit: usize) -> usize {
    bit / word_bits()
}

fn bitpos(bit: usize) -> usize {
    1 << (bit % word_bits())
}

impl BitVec {
    pub fn new(bits: usize) -> Self {
        let word_count = (bits + word_bits() - 1) / word_bits();
        let mut words = Vec::with_capacity(word_count);
        for _ in 0..word_count {
            words.push(0);
        }

        BitVec {
            bits,
            words,
            first_clear: 0,
        }
    }

    pub fn is_set(&self, bit: usize) -> bool {
        self.words[idx(bit)] & bitpos(bit) != 0
    }

    pub fn first_clear(&self) -> usize {
        self.first_clear
    }

    pub fn set(&mut self, bit: usize) {
        self.words[idx(bit)] |= bitpos(bit);
        if bit == self.first_clear {
            self.first_clear += 1;
            while self.is_set(self.first_clear) && self.first_clear < self.bits {
                self.first_clear += 1;
            }
        }
    }

    pub fn clear(&mut self, bit: usize) {
        self.words[idx(bit)] &= !bitpos(bit);
        if bit < self.first_clear {
            self.first_clear = bit;
        }
    }
}