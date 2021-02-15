/*
 * Copyright (C) 2015-2020, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Copyright (C) 2019-2020, Tendsin Mende <tendsin@protonmail.com>
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

use crate::buf::MetaBufferBlockRef;
use crate::data::{BlockNo, INodeRef};

use core::fmt;
use core::u32;

use m3::cell::Cell;
use m3::mem::size_of;

/// Represents a file position given by extent number and offset within the extent
#[derive(Copy, Clone)]
pub struct ExtPos {
    pub ext: usize,
    pub off: usize,
}

impl ExtPos {
    pub fn new(ext: usize, off: usize) -> Self {
        Self { ext, off }
    }

    pub fn next_ext(&mut self) {
        self.ext += 1;
        self.off = 0;
    }
}

impl fmt::Debug for ExtPos {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ExtPos[ext={}, off={}]", self.ext, self.off)
    }
}

/// Represents an extent as stored on disk
#[derive(Clone, Copy, Debug)]
#[repr(C, align(8))]
pub struct Extent {
    pub start: u32,
    pub length: u32,
}

impl Extent {
    pub fn new(start: u32, length: u32) -> Self {
        Self { start, length }
    }

    pub fn block_range(&self) -> core::ops::Range<BlockNo> {
        core::ops::Range {
            start: self.start,
            end: self.start + self.length,
        }
    }

    pub fn block_iter(&self) -> BlockIterator {
        BlockIterator {
            range: self.block_range(),
        }
    }
}

/// A reference to an direct or indirect extent
pub struct ExtentRef {
    block_ref: MetaBufferBlockRef,
    // this pointer is valid during our lifetime, because we keep a MetaBufferBlockRef
    extent: *mut Extent,
    dirty: Cell<bool>,
}

impl Clone for ExtentRef {
    fn clone(&self) -> Self {
        Self {
            block_ref: self.block_ref.clone(),
            extent: self.extent,
            // we reference the same block; there is no need to mark it dirty twice
            dirty: Cell::from(false),
        }
    }
}

impl ExtentRef {
    /// Loads the extent with given index from the given INode
    pub fn dir_from_inode(inode: &INodeRef, index: usize) -> Self {
        Self {
            block_ref: inode.block().clone(),
            extent: &mut inode.as_mut().direct[index],
            dirty: Cell::from(false),
        }
    }

    /// Loads the indirect extent at given offset from given MetaBufferBlock
    pub fn indir_from_buffer(block_ref: MetaBufferBlockRef, off: usize) -> Self {
        let block = crate::hdl().metabuffer().get_block_by_ref(&block_ref);
        debug_assert!(
            off % size_of::<Extent>() == 0,
            "Extent off is not multiple of extent size!"
        );
        debug_assert!(
            off + size_of::<Extent>() <= block.data().len(),
            "Extent off exceeds block!"
        );

        // safety: the cast is valid if the checks above succeeded
        let ext = unsafe {
            let mem = block.data_mut().as_mut_ptr();
            mem.cast::<Extent>().add(off / size_of::<Extent>())
        };

        Self {
            block_ref: block_ref.clone(),
            extent: ext,
            dirty: Cell::from(false),
        }
    }

    #[allow(clippy::mut_from_ref)]
    pub fn as_mut(&self) -> &mut Extent {
        self.dirty.set(true);
        // safety: valid because we keep a MetaBufferBlockRef
        unsafe { &mut *self.extent }
    }
}

impl core::ops::Deref for ExtentRef {
    type Target = Extent;

    fn deref(&self) -> &Self::Target {
        // safety: valid because we keep a MetaBufferBlockRef
        unsafe { &*self.extent }
    }
}

impl Drop for ExtentRef {
    fn drop(&mut self) {
        if self.dirty.get() {
            self.block_ref.mark_dirty();
        }
    }
}

/// Block iterator that delivers all blocks of an extent.
pub struct BlockIterator {
    range: core::ops::Range<BlockNo>,
}

impl core::iter::Iterator for BlockIterator {
    type Item = MetaBufferBlockRef;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.range.is_empty() {
            self.range.start += 1;
            crate::hdl()
                .metabuffer()
                .get_block(self.range.start - 1)
                .ok()
        }
        else {
            None
        }
    }
}

/// A cache for a block of extents
pub struct ExtentCache {
    block_ref: MetaBufferBlockRef,
    // this pointer is valid during our lifetime, because we keep a MetaBufferBlockRef
    extents: *const Extent,
}

impl ExtentCache {
    pub fn from_buffer(block_ref: MetaBufferBlockRef) -> Self {
        let block = crate::hdl().metabuffer().get_block_by_ref(&block_ref);
        let extents = block.data().as_ptr().cast::<Extent>();
        Self { block_ref, extents }
    }

    pub fn get_ref(&self, idx: usize) -> ExtentRef {
        ExtentRef::indir_from_buffer(self.block_ref.clone(), idx * size_of::<Extent>())
    }
}

impl core::ops::Index<usize> for ExtentCache {
    type Output = Extent;

    fn index(&self, idx: usize) -> &Self::Output {
        assert!(idx < crate::hdl().superblock().extents_per_block());
        // safety: valid because we keep a MetaBufferBlockRef
        unsafe { &*self.extents.add(idx) }
    }
}
