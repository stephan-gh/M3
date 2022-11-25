/*
 * Copyright (C) 2020-2021 Nils Asmussen, Barkhausen Institut
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

use m3::col::String;

use crate::data::bitmap::Bitmap;

use m3::errors::{Code, Error};

#[derive(Debug)]
pub struct Allocator {
    name: String,
    first: u32,
    first_free: u32,
    free: u32,
    total: u32,
    blocks: u32,
    blocksize: usize,
}

impl Allocator {
    pub fn new(
        name: String,
        first: u32,
        first_free: u32,
        free: u32,
        total: u32,
        blocks: u32,
        blocksize: usize,
    ) -> Self {
        let alloc = Allocator {
            name,
            first,
            first_free,
            free,
            total,
            blocks,
            blocksize,
        };
        log!(crate::LOG_ALLOC, "Created {:#?}", alloc);
        alloc
    }

    pub fn first_free(&self) -> u32 {
        self.first_free
    }

    pub fn free_count(&self) -> u32 {
        self.free
    }

    pub fn alloc(&mut self, count: Option<&mut usize>) -> Result<u32, Error> {
        let mut tmp_count = 1;
        let count = count.unwrap_or(&mut tmp_count);

        let perblock: usize = self.blocksize * 8;
        let lastno: u32 = self.first + self.blocks - 1;

        let icount = *count;

        let mut no: u32 = (self.first as usize + self.first_free as usize / perblock) as u32;
        let mut total: usize = 0;
        let mut i = (self.first_free as usize) % perblock;

        while (total == 0) && (no <= lastno) {
            let mut block = crate::meta_buffer_mut().get_block(no)?;
            block.mark_dirty();

            // take care that total_blocks might not be a multiple of perblock
            let mut max = perblock;
            if no == lastno {
                max = (self.total as usize) % perblock;
                max = if max == 0 { perblock } else { max };
            }

            // load data into bitmap
            let mut bitmap = Bitmap::from_bytes(block.data_mut());

            // first, search quickly until we've found a word that has free bits
            if i < max && bitmap.is_word_set(i) {
                // within the first word with free bits, the first free bit is not necessarily
                // i % Bitmap::WORD_BITS. thus, start from 0 within each word
                i = (i + Bitmap::word_size()) & !(Bitmap::word_size() - 1);
                while i < max && bitmap.is_word_set(i) {
                    i += Bitmap::word_size();
                }
                if i < max {
                    // now walk to the actual first free bit
                    while i < max && bitmap.is_bit_set(i) {
                        i += 1;
                    }
                }
            }

            // now walk until its aligned (i < max is not required here since a block is always a multiple
            // of Bitmap::WORD_BITS and we run only until i % Bitmap::WORD_BITS == 0)
            while ((i % Bitmap::word_size()) != 0) && total < icount {
                if !bitmap.is_bit_set(i) {
                    bitmap.set_bit(i);
                    total += 1;
                }
                else if total > 0 {
                    break;
                }

                i += 1;
            }

            // at this point i is aligned to the word size, now allocate in words
            while ((icount - total) >= Bitmap::word_size()) && ((max - i) >= Bitmap::word_size()) {
                if bitmap.is_word_unset(i) {
                    bitmap.set_word(i);
                    total += Bitmap::word_size();
                }
                else if total > 0 {
                    break;
                }

                i += Bitmap::word_size();
            }

            // set the bits that are left one bit at a time
            if total == 0 {
                while (i < max) && (total < icount) {
                    if !bitmap.is_bit_set(i) {
                        bitmap.set_bit(i);
                        total += 1;
                    }

                    i += 1;
                }
            }
            else {
                while (i < max) && (total < icount) && !bitmap.is_bit_set(i) {
                    bitmap.set_bit(i);
                    total += 1;
                    i += 1;
                }
            }

            if total == 0 {
                no += 1;
                i = 0;
            }
        }

        // finally mark the allocated bits in the superblock
        assert!(
            self.free as usize >= total,
            "tried to allocate more than available according to superblock!"
        );

        self.free -= total as u32;
        *count = total; // notify caller about the number of allocated items
        if total == 0 {
            return Err(Error::new(Code::NoSpace));
        }

        let off = (no - self.first) * perblock as u32 + i as u32;
        self.first_free = off;

        let start = off - total as u32;
        log!(
            crate::LOG_ALLOC,
            "allocator[{}]::alloc(count={}) -> {}..{}",
            self.name,
            count,
            start,
            (start + total as u32 - 1)
        );

        Ok(start)
    }

    pub fn free(&mut self, mut start: usize, mut count: usize) -> Result<(), Error> {
        log!(
            crate::LOG_ALLOC,
            "allocator[{}]::free(start={}, count={})",
            self.name,
            start,
            count
        );

        let perblock: usize = self.blocksize * 8;
        let mut no: usize = self.first as usize + start / perblock;

        if start < self.first_free as usize {
            self.first_free = start as u32;
        }
        self.free += count as u32;
        while count > 0 {
            let mut block = crate::meta_buffer_mut().get_block(no as u32)?;
            block.mark_dirty();
            let mut bitmap = Bitmap::from_bytes(block.data_mut());

            // first, align it to word-size
            let mut i: usize = start & (perblock - 1);
            let begin = i;
            let end = (i + count).min(perblock);

            // Unset all unaligned bits
            while i < end && (i % Bitmap::word_size()) != 0 {
                assert!(bitmap.is_bit_set(i));
                bitmap.unset_bit(i);
                i += 1;
            }

            // now clear in word-steps
            let wend = end & (!(Bitmap::word_size() - 1));
            while i < wend {
                assert!(bitmap.is_word_set(i));
                bitmap.unset_word(i);

                i += Bitmap::word_size();
            }

            // maybe, there is something left
            while i < end {
                assert!(bitmap.is_bit_set(i));
                bitmap.unset_bit(i);
                i += 1;
            }

            // to next bitmap block
            count -= i - begin;
            start = (start + perblock - 1) & !(perblock - 1);
            no += 1;
        }

        Ok(())
    }
}
