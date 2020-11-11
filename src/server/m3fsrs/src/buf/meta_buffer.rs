/*
 * Copyright (C) 2015-2020, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Copyright (C) 2019-2020, Tendsin Mende <tendsin@protonmail.com>
 * Copyright (C) 2018, Sebastian Reimers <sebastian.reimers@mailbox.tu-dresden.de>
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

use crate::buf::Buffer;
use crate::data::BlockNo;

use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;

use m3::boxed::Box;
use m3::col::{BoxList, Treap, Vec};
use m3::errors::Error;

use thread::Event;

/// A single block in the meta buffer
pub struct MetaBufferBlock {
    id: usize,
    bno: BlockNo,

    prev: Option<NonNull<Self>>,
    next: Option<NonNull<Self>>,

    locked: bool,
    dirty: bool,
    links: usize,
    unlock: Event,

    data: Vec<u8>,
}

impl_boxitem!(MetaBufferBlock);

pub const META_BUFFER_SIZE: usize = 128;

impl MetaBufferBlock {
    fn new(id: usize, bno: BlockNo, blocksize: usize) -> Self {
        MetaBufferBlock {
            id,
            bno,

            prev: None,
            next: None,

            locked: true,
            dirty: false,
            links: 0,
            unlock: thread::ThreadManager::get().alloc_event(),

            data: vec![0; blocksize as usize],
        }
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// Overwrites the data of this block with zeros
    pub fn overwrite_zero(&mut self) {
        for i in &mut self.data {
            *i = 0;
        }
    }
}

pub struct MetaBufferBlockRef {
    id: usize,
}

impl MetaBufferBlockRef {
    fn new(id: usize) -> Self {
        let mut r = Self { id };
        r.links += 1;
        r
    }
}

impl Clone for MetaBufferBlockRef {
    fn clone(&self) -> Self {
        Self::new(self.id)
    }
}

impl Deref for MetaBufferBlockRef {
    type Target = MetaBufferBlock;

    fn deref(&self) -> &Self::Target {
        crate::hdl().metabuffer().get_block_by_id(self.id)
    }
}

impl DerefMut for MetaBufferBlockRef {
    fn deref_mut(&mut self) -> &mut Self::Target {
        crate::hdl().metabuffer().get_block_mut_by_id(self.id)
    }
}

impl Drop for MetaBufferBlockRef {
    fn drop(&mut self) {
        let block = self.deref_mut();
        assert!(block.links > 0);
        block.links -= 1;
    }
}

pub struct MetaBuffer {
    // contains the actual MetaBufferBlock objects and keeps them sorted by LRU
    lru: BoxList<MetaBufferBlock>,
    // gives us a quick translation from block number to block id (index in the following vector)
    ids: Treap<BlockNo, usize>,
    // contains pointers to the MetaBufferBlock objects, indexed by their id
    blocks: Vec<NonNull<MetaBufferBlock>>,
}

impl MetaBuffer {
    pub fn new(blocksize: usize) -> Self {
        let mut blocks = Vec::with_capacity(META_BUFFER_SIZE);
        let mut lru = BoxList::new();
        for i in 0..META_BUFFER_SIZE {
            let mut buffer = Box::new(MetaBufferBlock::new(i, 0, blocksize));
            // we can store the pointer in the vector, because boxing prevents it from moving.
            unsafe {
                blocks.push(NonNull::new_unchecked(&mut *buffer as *mut _));
            }
            lru.push_back(buffer);
        }

        MetaBuffer {
            ids: Treap::new(),
            blocks,
            lru,
        }
    }

    fn bno_to_id(&self, bno: BlockNo) -> Option<usize> {
        self.ids.get(&bno).map(|id| *id)
    }

    fn get_block_by_id(&self, id: usize) -> &MetaBufferBlock {
        unsafe { &(*self.blocks[id].as_ptr()) }
    }

    fn get_block_mut_by_id(&mut self, id: usize) -> &mut MetaBufferBlock {
        unsafe { &mut (*self.blocks[id].as_mut()) }
    }

    pub fn get_block_by_ref(&mut self, r: &MetaBufferBlockRef) -> &mut MetaBufferBlock {
        self.get_block_mut_by_id(r.id)
    }

    /// Searches for data at `bno`, allocates if none is present.
    pub fn get_block(&mut self, bno: BlockNo, dirty: bool) -> Result<MetaBufferBlockRef, Error> {
        log!(
            crate::LOG_BUFFER,
            "metabuffer::get_block(bno={}, dirty={})",
            bno,
            dirty
        );

        loop {
            if let Some(id) = self.bno_to_id(bno) {
                // workaround for borrow-checker: don't use our convenience function
                let block = unsafe { &mut (*self.blocks[id].as_ptr()) };

                if block.locked {
                    thread::ThreadManager::get().wait_for(block.unlock);
                }
                else {
                    // move element to back since it was touched
                    unsafe {
                        self.lru.move_to_back(block);
                    }
                    block.dirty |= dirty;

                    log!(
                        crate::LOG_BUFFER,
                        "metabuffer: found cached block <{}>, links: {}",
                        block.bno,
                        block.links + 1,
                    );
                    return Ok(MetaBufferBlockRef::new(block.id));
                }
            }
            else {
                // no block for block number, therefore allocate
                break;
            }
        }

        // find first unused head
        let mut use_block = None;
        for lru_element in self.lru.iter() {
            if lru_element.links == 0 {
                use_block = Some(lru_element.id);
                break;
            }
        }

        let block = unsafe {
            let block = &mut (*self.blocks[use_block.unwrap()].as_ptr());
            self.lru.move_to_back(block);
            block
        };

        // flush if there is still a block present with the given bno.
        if block.bno != 0 {
            self.ids.remove(&block.bno);
            if block.dirty {
                Self::flush_chunk(block)?;
            }
        }

        // use this block
        block.bno = bno;
        self.ids.insert(bno, block.id);

        let unlock = block.unlock;
        // now load from backend and setup everything
        crate::hdl()
            .backend()
            .load_meta(block, block.id, bno, unlock)?;
        block.dirty = dirty;
        block.locked = false;

        log!(
            crate::LOG_BUFFER,
            "metabuffer: loaded new block<{}> links: {}",
            bno,
            block.links + 1,
        );
        Ok(MetaBufferBlockRef::new(block.id))
    }

    pub fn dirty(&self, bno: BlockNo) -> bool {
        if let Some(b) = self.get(bno) {
            b.dirty
        }
        else {
            false
        }
    }

    pub fn write_back(&mut self, bno: BlockNo) -> Result<(), Error> {
        if let Some(b) = self.get_mut(bno) {
            if b.dirty {
                Self::flush_chunk(b)?;
            }
        }
        Ok(())
    }
}

impl Buffer for MetaBuffer {
    type HEAD = MetaBufferBlock;

    fn mark_dirty(&mut self, bno: BlockNo) {
        if let Some(b) = self.get_mut(bno) {
            b.dirty = true;
        }
    }

    fn flush(&mut self) -> Result<(), Error> {
        for block_ptr in &mut self.blocks {
            let block = unsafe { &mut (*block_ptr.as_ptr()) };
            if block.dirty {
                Self::flush_chunk(block)?;
            }
        }
        Ok(())
    }

    fn get(&self, bno: BlockNo) -> Option<&Self::HEAD> {
        self.bno_to_id(bno).map(|id| &*self.get_block_by_id(id))
    }

    fn get_mut(&mut self, bno: BlockNo) -> Option<&mut Self::HEAD> {
        self.bno_to_id(bno)
            .map(|id| unsafe { &mut (*self.blocks[id].as_ptr()) })
    }

    fn flush_chunk(head: &mut Self::HEAD) -> Result<(), Error> {
        head.locked = true;
        log!(
            crate::LOG_BUFFER,
            "metabuffer: writing back block <{}>",
            head.bno
        );

        // write meta block to backend
        crate::hdl()
            .backend()
            .store_meta(head, head.id, head.bno, head.unlock)?;
        head.dirty = false;
        head.locked = false;
        Ok(())
    }
}
