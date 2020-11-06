use crate::buffer::Buffer;
use crate::internal::*;
use crate::sess::request::Request;

use core::ptr::NonNull;
use m3::boxed::Box;
use m3::col::{BoxList, Treap, Vec};
use m3::errors::Error;

use thread::Event;

/// The BufferHead represents a single block within the MetaBuffer. In Rust, contrary to C++ it also
/// handles runtime type checking. When loading an Inode, extent, or DirEntry from it, it checks if this entity has already been loaded.
/// If that's the case, a reference to this entity is returned, otherwise a new entity is loaded and returned.
pub struct MetaBufferHead {
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

impl_boxitem!(MetaBufferHead);

impl core::cmp::PartialEq for MetaBufferHead {
    fn eq(&self, other: &Self) -> bool {
        self.bno == other.bno
    }
}

pub const META_BUFFER_SIZE: usize = 128;

impl MetaBufferHead {
    pub fn new(id: usize, bno: BlockNo, blocksize: usize) -> Self {
        MetaBufferHead {
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

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    // Overwrites the data of this block with zeros
    pub fn overwrite_zero(&mut self) {
        for i in &mut self.data {
            *i = 0;
        }
    }
}

pub struct MetaBuffer {
    /// Contains the actual MetaBufferHead objects and keeps them sorted by LRU
    lru: BoxList<MetaBufferHead>,
    /// Gives us a quick translation from block number to block id (index in the following vector)
    ht: Treap<BlockNo, usize>,
    /// Contains pointers to the MetaBufferHead objects, indexed by their id
    blocks: Vec<NonNull<MetaBufferHead>>,
}

impl MetaBuffer {
    pub fn new(blocksize: usize) -> Self {
        let mut blocks = Vec::with_capacity(META_BUFFER_SIZE);
        let mut lru = BoxList::new();
        for i in 0..META_BUFFER_SIZE {
            let mut buffer = Box::new(MetaBufferHead::new(i, 0, blocksize));
            // we can store the pointer in the vector, because boxing prevents it from moving.
            unsafe {
                blocks.push(NonNull::new_unchecked(&mut *buffer as *mut _));
            }
            lru.push_back(buffer);
        }

        MetaBuffer {
            ht: Treap::new(),
            blocks,
            lru,
        }
    }

    fn bno_to_id(&self, bno: BlockNo) -> Option<usize> {
        self.ht.get(&bno).map(|id| *id)
    }

    fn get_block_by_id(&self, id: usize) -> &MetaBufferHead {
        unsafe { &(*self.blocks[id].as_ptr()) }
    }

    pub fn get_block_mut_by_id(&mut self, id: usize) -> &mut MetaBufferHead {
        unsafe { &mut (*self.blocks[id].as_ptr()) }
    }

    /// Searches for data at `bno`, allocates if none is present.
    pub fn get_block(
        &mut self,
        req: &mut Request,
        bno: BlockNo,
        dirty: bool,
    ) -> Result<&mut MetaBufferHead, Error> {
        log!(
            crate::LOG_DEF,
            "MetaBuffer::get_block(bno={}, dirty={})",
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
                    // Move element to back since it was touched
                    unsafe {
                        self.lru.move_to_back(block);
                    }
                    block.dirty |= dirty;
                    block.links += 1;

                    log!(
                        crate::LOG_DEF,
                        "MetaBuffer: Found cached block <{}>, Links: {}",
                        block.bno,
                        block.links,
                    );
                    req.push_meta(block.id);
                    return Ok(block);
                }
            }
            else {
                // No block for block number, therefore allocate
                break;
            }
        }

        let mut use_block = None;
        // Find first unused head
        for lru_element in self.lru.iter() {
            if lru_element.links == 0 {
                // Only saved in lru and ht but unused
                use_block = Some(lru_element.id);
                break;
            }
        }

        let block = unsafe {
            let block = &mut (*self.blocks[use_block.unwrap()].as_ptr());
            self.lru.move_to_back(block);
            block
        };

        // Flush if there is still a block present with the given bno.
        if block.bno != 0 {
            self.ht.remove(&block.bno);
            if block.dirty {
                Self::flush_chunk(block)?;
            }
        }

        // Now we are save to use this bno
        // Insert into ht
        block.bno = bno;
        self.ht.insert(bno, block.id);

        let unlock = block.unlock;
        // Now load from backend and setup everything
        crate::hdl()
            .backend()
            .load_meta(block, block.id, bno, unlock)?;
        block.dirty = dirty;
        block.locked = false;
        block.links += 1;

        log!(
            crate::LOG_DEF,
            "MetaBuffer: Load new block<{}> Links: {}",
            bno,
            block.links,
        );
        req.push_meta(block.id);
        Ok(block)
    }

    pub fn deref(&mut self, id: usize) {
        let block = self.get_block_mut_by_id(id);
        assert!(block.links > 0);
        block.links -= 1;
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
    type HEAD = MetaBufferHead;

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
            crate::LOG_DEF,
            "MetaBuffer: Write back block <{}>",
            head.bno
        );

        // Write meta block to backend device
        crate::hdl()
            .backend()
            .store_meta(head, head.id, head.bno, head.unlock)?;
        head.dirty = false;
        head.locked = false;
        Ok(())
    }
}
