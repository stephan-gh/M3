use crate::backend::Backend;
use crate::buffer::{Buffer, PRDT_SIZE};
use crate::internal::BlockNo;

use core::cmp;
use core::fmt;
use core::ptr::NonNull;

use m3::boxed::Box;
use m3::cap::Selector;
use m3::col::{BoxList, Treap};
use m3::com::{MemGate, Perm};
use m3::errors::Error;

use thread::Event;

#[derive(Copy, Clone, PartialOrd, PartialEq, Eq)]
pub struct BlockRange {
    start: BlockNo,
    count: BlockNo,
}

impl BlockRange {
    pub fn new(bno: BlockNo) -> Self {
        Self::new_range(bno, 1)
    }

    pub fn new_range(start: BlockNo, count: BlockNo) -> Self {
        BlockRange { start, count }
    }
}

impl fmt::Debug for BlockRange {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.start + self.count - 1)
    }
}

impl cmp::Ord for BlockRange {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        if self.start >= other.start && self.start < other.start + other.count {
            cmp::Ordering::Equal
        }
        else if self.start < other.start {
            cmp::Ordering::Less
        }
        else {
            cmp::Ordering::Greater
        }
    }
}

pub struct FileBufferEntry {
    blocks: BlockRange,
    data: m3::com::MemGate,

    prev: Option<NonNull<Self>>,
    next: Option<NonNull<Self>>,

    locked: bool,
    dirty: bool,
    unlock: Event,
}

impl_boxitem!(FileBufferEntry);

impl FileBufferEntry {
    fn new(blocks: BlockRange, blocksize: usize) -> Result<Self, Error> {
        Ok(FileBufferEntry {
            blocks,
            data: MemGate::new(blocks.count as usize * blocksize + PRDT_SIZE, Perm::RWX)?,

            prev: None,
            next: None,

            locked: true,
            dirty: false,
            unlock: thread::ThreadManager::get().alloc_event(),
        })
    }
}

pub struct FileBuffer {
    size: usize,

    // contains the actual FileBufferEntry objects and keeps them sorted by LRU
    lru: BoxList<FileBufferEntry>,
    // gives us a quick translation from block number to FileBufferEntry
    entries: Treap<BlockRange, NonNull<FileBufferEntry>>,

    block_size: usize,
}

pub const FILE_BUFFER_SIZE: usize = 16384;

impl FileBuffer {
    pub fn new(block_size: usize) -> Self {
        FileBuffer {
            size: 0,

            lru: BoxList::new(),
            entries: Treap::new(),

            block_size,
        }
    }

    pub fn get_extent(
        &mut self,
        backend: &dyn Backend,
        bno: BlockNo,
        size: usize,
        sel: Selector,
        perm: Perm,
        accessed: usize,
        load: Option<bool>,
    ) -> Result<usize, Error> {
        let load = load.unwrap_or(true);

        log!(
            crate::LOG_BUFFER,
            "filebuffer::get_extent(bno={}, size={}, sel={}, load={})",
            bno,
            size,
            sel,
            load,
        );

        loop {
            // workaround for borrow-checker: don't use our convenience function
            let block_opt = self
                .entries
                .get_mut(&BlockRange::new(bno))
                .map(|b| unsafe { &mut *b.as_mut() });

            if let Some(head) = block_opt {
                let start = head.blocks.start;

                if head.locked {
                    // wait for block to unlock
                    log!(
                        crate::LOG_BUFFER,
                        "filebuffer: waiting for cached blocks <{:?}>",
                        head.blocks,
                    );
                    thread::ThreadManager::get().wait_for(head.unlock);
                }
                else {
                    // move element to back since it was touched
                    unsafe {
                        self.lru.move_to_back(head);
                    }

                    log!(
                        crate::LOG_BUFFER,
                        "filebuffer: found cached blocks <{:?}>",
                        head.blocks,
                    );

                    let len = size.min((head.blocks.count - (bno - start)) as usize);
                    m3::syscalls::derive_mem(
                        m3::pes::VPE::cur().sel(),
                        sel,
                        head.data.sel(),
                        ((bno - start) as u64) * self.block_size as u64,
                        len * self.block_size,
                        perm,
                    )?;

                    head.dirty |= perm.contains(Perm::W);

                    return Ok(len * self.block_size);
                }
            }
            else {
                break;
            }
        }

        // load chunk into memory
        let max_size: usize = FILE_BUFFER_SIZE.min((1 as usize) << accessed);
        let load_size: usize = size.min(if load { max_size } else { FILE_BUFFER_SIZE });

        if (self.size + load_size) > FILE_BUFFER_SIZE {
            while (self.size + load_size) > FILE_BUFFER_SIZE {
                // remove oldest entry
                let mut head = self.lru.pop_front().unwrap();

                if head.locked {
                    // wait for block to be evicted
                    log!(
                        crate::LOG_BUFFER,
                        "filebuffer: waiting for eviction of blocks <{:?}>",
                        head.blocks,
                    );
                    thread::ThreadManager::get().wait_for(head.unlock);
                }
                else {
                    // remove from treap
                    log!(
                        crate::LOG_BUFFER,
                        "filebuffer: evict blocks <{:?}>",
                        head.blocks
                    );
                    self.entries.remove(&head.blocks);

                    // write it back, if it's dirty
                    if head.dirty {
                        Self::flush_chunk(&mut head).unwrap();
                    }

                    // revoke access from clients
                    // TODO currently, clients are not prepared for that
                    m3::pes::VPE::cur()
                        .revoke(
                            m3::kif::CapRngDesc::new(m3::kif::CapType::OBJECT, head.data.sel(), 1),
                            false,
                        )
                        .unwrap();

                    // we have more space now
                    self.size -= head.blocks.count as usize;
                }
            }
        }

        // create new entry (boxed to ensure its pointer stays constant)
        let mut new_head = Box::new(FileBufferEntry::new(
            BlockRange::new_range(bno, load_size as BlockNo),
            self.block_size as usize,
        )?);
        self.size += new_head.blocks.count as usize;

        log!(
            crate::LOG_BUFFER,
            "filebuffer: allocated blocks <{:?}>{}",
            new_head.blocks,
            if load { " (loading)" } else { "" }
        );

        // load data from backend
        backend.load_data(
            &new_head.data,
            new_head.blocks.start,
            new_head.blocks.count as usize,
            load,
            new_head.unlock,
        )?;
        new_head.locked = false;

        m3::syscalls::derive_mem(
            m3::pes::VPE::cur().sel(),
            sel,
            new_head.data.sel(),
            0,
            load_size * self.block_size,
            perm,
        )?;

        new_head.dirty = perm.contains(Perm::W);

        // everything went fine, so insert pointer into treap and the object into the LRU list
        let ptr = unsafe { NonNull::new_unchecked(&mut *new_head as *mut _) };
        self.entries.insert(new_head.blocks, ptr);
        self.lru.push_back(new_head);

        Ok(load_size * self.block_size)
    }
}

impl Buffer for FileBuffer {
    type HEAD = FileBufferEntry;

    fn mark_dirty(&mut self, bno: BlockNo) {
        if let Some(b) = self.get_mut(bno) {
            b.dirty = true;
        }
    }

    fn flush(&mut self) -> Result<(), Error> {
        while let Some(mut b) = self.lru.pop_front() {
            self.entries.remove(&b.blocks);
            if b.dirty {
                Self::flush_chunk(&mut b)?;
            }
        }

        Ok(())
    }

    fn get(&self, bno: BlockNo) -> Option<&FileBufferEntry> {
        self.entries
            .get(&BlockRange::new(bno))
            .map(|b| unsafe { &*b.as_ptr() })
    }

    fn get_mut(&mut self, bno: BlockNo) -> Option<&mut FileBufferEntry> {
        self.entries
            .get_mut(&BlockRange::new(bno))
            .map(|b| unsafe { &mut *b.as_mut() })
    }

    fn flush_chunk(head: &mut FileBufferEntry) -> Result<(), Error> {
        head.locked = true;
        log!(
            crate::LOG_BUFFER,
            "filebuffer: writing back blocks <{:?}>",
            head.blocks,
        );

        // write data of block to backend
        crate::hdl().backend().store_data(
            head.blocks.start,
            head.blocks.count as usize,
            head.unlock,
        )?;

        // reset dirty and unlock
        head.dirty = false;
        head.locked = false;
        Ok(())
    }
}
