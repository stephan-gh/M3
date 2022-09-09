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

use crate::buf::MetaBufferBlock;
use crate::data::{InodeNo, DIR_ENTRY_LEN};

use core::intrinsics::transmute;
use core::slice;
use core::u32;

use m3::cell::Cell;
use m3::libc;
use m3::mem::size_of;
use m3::util::math;

/// On-disk representation of directory entries.
#[repr(align(4), C)]
pub struct DirEntry {
    pub nodeno: InodeNo,
    pub name_length: u32,
    pub next: u32,
    name: [i8],
}

macro_rules! get_entry_mut {
    ($buffer_off:expr) => {{
        // TODO ensure that name_length and next are within bounds (in case FS image is corrupt)
        let name_length = $buffer_off.add(size_of::<InodeNo>()) as *const u32;
        let slice = [$buffer_off as usize, *name_length as usize + DIR_ENTRY_LEN];
        transmute(slice)
    }};
}

impl DirEntry {
    /// Returns a reference to the directory entry stored at `off` in the given buffer
    pub fn from_buffer(block_data: &[u8], off: usize) -> &Self {
        unsafe {
            let buffer_off = block_data.as_ptr().add(off);
            get_entry_mut!(buffer_off)
        }
    }

    /// Returns a mutable reference to the directory entry stored at `off` in the given buffer
    pub fn from_buffer_mut(block: &mut MetaBufferBlock, off: usize) -> &mut Self {
        block.mark_dirty();
        unsafe {
            let buffer_off = block.data_mut().as_mut_ptr().add(off);
            get_entry_mut!(buffer_off)
        }
    }

    /// Returns a mutable reference to the directory entry stored at `off` in the given buffer
    pub fn two_from_buffer_mut(
        block: &mut MetaBufferBlock,
        off1: usize,
        off2: usize,
    ) -> (&mut Self, &mut Self) {
        assert!(off1 != off2);
        block.mark_dirty();
        unsafe {
            let buffer_off1 = block.data_mut().as_mut_ptr().add(off1);
            let buffer_off2 = block.data_mut().as_mut_ptr().add(off2);
            (get_entry_mut!(buffer_off1), get_entry_mut!(buffer_off2))
        }
    }

    /// Returns the size of this entry when stored on disk. Includes the static size of the struct
    /// as well as the str. buffer size.
    pub fn size(&self) -> usize {
        // make sure the next entry is 4-byte aligned
        DIR_ENTRY_LEN + math::round_up(self.name_length as usize, 4)
    }

    /// Returns the name of the entry
    pub fn name(&self) -> &str {
        unsafe {
            let sl = slice::from_raw_parts(self.name.as_ptr(), self.name_length as usize);
            &*(&sl[..sl.len()] as *const [i8] as *const str)
        }
    }

    /// Sets the name of the entry to the given one
    pub fn set_name(&mut self, name: &str) {
        self.name_length = name.len() as u32;
        unsafe {
            libc::memcpy(
                self.name.as_mut_ptr() as *mut libc::c_void,
                name.as_ptr() as *const libc::c_void,
                name.len(),
            );
        }
    }
}

/// Entry iterator takes a block and iterates over it assuming that the block contains entries.
pub struct DirEntryIterator<'e> {
    block_data: &'e [u8],
    off: Cell<usize>,
    end: usize,
}

impl<'e> DirEntryIterator<'e> {
    pub fn from_block(block_data: &'e [u8]) -> Self {
        DirEntryIterator {
            block_data,
            off: Cell::from(0),
            end: crate::superblock().block_size as usize,
        }
    }

    /// Returns the next DirEntry
    pub fn next(&'e self) -> Option<&'e DirEntry> {
        if self.off.get() < self.end {
            let ret = DirEntry::from_buffer(self.block_data, self.off.get());

            self.off.set(self.off.get() + ret.next as usize);

            Some(ret)
        }
        else {
            None
        }
    }
}
