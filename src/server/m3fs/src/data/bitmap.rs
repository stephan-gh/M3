/*
 * Copyright (C) 2020 Nils Asmussen, Barkhausen Institut
 * Copyright (C) 2019-2020, Tendsin Mende <tendsin@protonmail.com>
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

use core::fmt;

/// Bitmap wrapper for a number of bytes.
pub struct Bitmap<'a> {
    bytes: &'a mut [u8],
}

impl<'a> Bitmap<'a> {
    pub fn from_bytes(bytes: &'a mut [u8]) -> Bitmap<'a> {
        Bitmap { bytes }
    }

    /// Returns the index of the first bit with the word in which `index` lays.
    pub fn get_first_index_of_word(index: usize) -> usize {
        (index / Bitmap::word_size()) * Bitmap::word_size()
    }

    /// Checks if the word at `index` is 255. If `false` is returned, some bit is 0 within this
    /// word.
    pub fn is_word_set(&self, mut index: usize) -> bool {
        index /= Bitmap::word_size();
        self.bytes[index] == core::u8::MAX
    }

    pub fn is_word_unset(&self, mut index: usize) -> bool {
        index /= Bitmap::word_size();
        self.bytes[index] == core::u8::MIN
    }

    pub fn set_word(&mut self, mut index: usize) {
        index /= Bitmap::word_size();
        self.bytes[index] = core::u8::MAX;
    }

    pub fn unset_word(&mut self, mut index: usize) {
        index /= Bitmap::word_size();
        self.bytes[index] = core::u8::MIN;
    }

    fn get_word_bit_index(index: usize) -> (usize, usize) {
        let word_index = index / Bitmap::word_size();
        let bit_index = index - Bitmap::get_first_index_of_word(index);

        (word_index, bit_index)
    }

    pub fn set_bit(&mut self, index: usize) {
        let (word_index, bit_index) = Bitmap::get_word_bit_index(index);
        self.bytes[word_index] |= 1 << bit_index;
    }

    pub fn unset_bit(&mut self, index: usize) {
        let (word_index, bit_index) = Bitmap::get_word_bit_index(index);
        self.bytes[word_index] &= core::u8::MAX ^ (1 << bit_index);
    }

    pub fn is_bit_set(&self, index: usize) -> bool {
        let (word_index, bit_index) = Bitmap::get_word_bit_index(index);
        ((self.bytes[word_index] >> bit_index) & 1) == 1
    }

    pub fn word_size() -> usize {
        8
    }
}

impl<'a> fmt::Debug for Bitmap<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        writeln!(f, "Bitmap[")?;
        for (idx, byte) in self.bytes.iter().enumerate() {
            writeln!(f, "  [{}]\t {:b}\t:{}", idx, byte, byte)?;
        }
        write!(f, "]")
    }
}
