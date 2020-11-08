use m3::col::String;

use crate::util::Bitmap;

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
        log!(crate::LOG_DEF, "Created {:#?}", alloc);
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

        let perblock: usize = self.blocksize as usize * 8;
        let lastno: u32 = self.first + self.blocks - 1;

        let icount = *count;

        let mut no: u32 = (self.first as usize + self.first_free as usize / perblock) as u32;
        let mut total: usize = 0;
        let mut i = (self.first_free as usize) % perblock;

        while (total == 0) && (no <= lastno) {
            let mut block = crate::hdl().metabuffer().get_block(no, true)?;

            let mut max = perblock;
            if no == lastno {
                max = (self.total as usize) % perblock;
                max = if max == 0 { perblock } else { max };
            }

            // Load data into bitmap
            let mut bitmap = Bitmap::from_bytes(block.data_mut());

            // Search for first word that has at leas one free bit, starting at the current i
            while i < max && bitmap.is_word_set(i) {
                i += Bitmap::word_size(); // Jump to next word
            }

            // Now we know i is in a word that has unset bits, since the bit is somewhere in the word, jump
            // back to the start of this word and iterate over the bits until we found the bit.

            // This should be the index of the word we found the first 0 at
            let word_index = i / Bitmap::word_size();
            i = word_index * Bitmap::word_size();
            while i < max && bitmap.is_bit_set(i) {
                i += 1;
            }

            // I should now point to the first unset index
            // Now set all bits until i is aligned to a whole word.
            while ((i % Bitmap::word_size()) != 0) && total < icount {
                if !bitmap.is_bit_set(i) {
                    bitmap.set_bit(i);
                    total += 1; // add bits to total allocated since we cant use them anymore
                }
                else if total > 0 {
                    break; // Not sure about this one, but it works and was in the reference impl
                }

                i += 1;
            }

            // At this point i is aligned to the word size, now mark all whole words
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

            // Now set the bit that are left (but not enough to fill a whole word)
            // there is an edge case where icount was < BitMap::word_size()
            // in that case total is at this point still 0
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

        // Finally mark the allocated bits in the superblock (which are shared with this allocator)
        assert!(
            self.free as usize >= total,
            "Error: Tried to allocate more then was available according to superblock!"
        );

        self.free -= total as u32;
        *count = total; // It happens that more was allocated then needed because of alignment
        if total == 0 {
            return Err(Error::new(Code::NoSpace));
        }

        let off = (no - self.first) * perblock as u32 + i as u32;
        self.first_free = off;

        let start = off - total as u32;
        log!(
            crate::LOG_DEF,
            "M3FS: {} allocated: {}..{}",
            self.name,
            start,
            (start + total as u32 - 1)
        );

        return Ok(start);
    }

    pub fn free(&mut self, mut start: usize, mut count: usize) -> Result<(), Error> {
        log!(
            crate::LOG_DEF,
            "Allocator::{}::free(start={}, count={})",
            self.name,
            start,
            count
        );

        let perblock: usize = self.blocksize as usize * 8;
        let mut no: usize = self.first as usize + start / perblock;

        if start < self.first_free as usize {
            self.first_free = start as u32;
        }
        self.free += count as u32;
        // Actually free bits in bitmap and update superblock
        while count > 0 {
            let mut block = crate::hdl().metabuffer().get_block(no as u32, true)?;
            let mut bitmap = Bitmap::from_bytes(block.data_mut());

            // align i to wordsize
            let mut i: usize = start & (perblock - 1);
            let begin = i;
            let end = (i + count).min(perblock);

            // Unset all unaligned bits
            while i < end && (i % Bitmap::word_size()) != 0 {
                assert!(bitmap.is_bit_set(i), "Bit should have been set!");
                bitmap.unset_bit(i);
                i += 1;
            }

            // Now clear all whole word
            let wend = end & (!(Bitmap::word_size() - 1));
            while i < wend {
                assert!(
                    bitmap.is_word_set(i),
                    "Word should have been set for clearing"
                );
                bitmap.unset_word(i);

                i += Bitmap::word_size();
            }
            // Clear possible rest
            while i < end {
                assert!(bitmap.is_bit_set(i), "Rest bit should have been set");
                bitmap.unset_bit(i);
                i += 1;
            }

            // Go to next bitmap block from rep
            count -= i - begin;
            start = (start + perblock - 1) & !(perblock - 1);
            no += 1;
        }

        log!(
            crate::LOG_DEF,
            "M3FS: {} free'd {}..{}",
            self.name,
            start,
            (start + count - 1)
        );
        Ok(())
    }
}
