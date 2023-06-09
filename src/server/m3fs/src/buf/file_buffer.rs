/*
 * Copyright (C) 2020-2022 Nils Asmussen, Barkhausen Institut
 * Copyright (C) 2019-2020, Tendsin Mende <tendsin@protonmail.com>
 * Copyright (C) 2018, Sebastian Reimers <sebastian.reimers@mailbox.tu-dresden.de>
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

use crate::backend::{Backend, PRDT_SIZE};
use crate::data::{BlockNo, BlockRange};

use core::cmp;
use core::fmt;
use core::ptr::NonNull;

use m3::boxed::Box;
use m3::cap::Selector;
use m3::col::{BoxList, Treap};
use m3::com::{MemGate, Perm};
use m3::errors::Error;
use m3::io::LogFlags;
use m3::mem::GlobOff;

use thread::Event;

pub const MAX_BUFFERED_BLKS: usize = 16384;
const MAX_BLKS_PER_ENTRY: usize = 1024;

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
            unlock: thread::alloc_event(),
        })
    }

    fn flush(&mut self) -> Result<(), Error> {
        if self.dirty {
            self.locked = true;
            log!(
                LogFlags::FSBuf,
                "filebuffer: writing back blocks <{:?}>",
                self.blocks,
            );

            // write data of block to backend
            crate::backend_mut().store_data(self.blocks, self.unlock)?;

            // reset dirty and unlock
            self.dirty = false;
            self.locked = false;
        }
        Ok(())
    }
}

pub struct LoadLimit {
    counter: usize,
}

impl LoadLimit {
    pub fn new() -> Self {
        Self { counter: 1 }
    }

    pub fn limit(&self) -> usize {
        cmp::min(1 << self.counter, MAX_BLKS_PER_ENTRY)
    }

    pub fn load(&mut self) -> usize {
        let limit = self.limit();
        if self.counter < 31 {
            self.counter += 1;
        }
        limit
    }
}

impl fmt::Debug for LoadLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LoadLimit={} blocks", self.limit())
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
        mut load: Option<&mut LoadLimit>,
    ) -> Result<usize, Error> {
        log!(
            LogFlags::FSBuf,
            "filebuffer::get_extent(bno={}, size={}, sel={}, load={:?})",
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
                        LogFlags::FSBuf,
                        "filebuffer: waiting for cached blocks <{:?}>",
                        head.blocks,
                    );
                    thread::wait_for(head.unlock);
                }
                else {
                    // move element to back since it was touched
                    unsafe {
                        self.lru.move_to_back(head);
                    }

                    log!(
                        LogFlags::FSBuf,
                        "filebuffer: found cached blocks <{:?}>",
                        head.blocks,
                    );

                    let len = size.min((head.blocks.count - (bno - start)) as usize);
                    m3::syscalls::derive_mem(
                        m3::tiles::Activity::own().sel(),
                        sel,
                        head.data.sel(),
                        ((bno - start) as u64) * self.block_size as u64,
                        (len * self.block_size) as GlobOff,
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

        // determine number of blocks to load
        let load_size: usize = size.min(if let Some(ref mut l) = load {
            l.load()
        }
        else {
            MAX_BLKS_PER_ENTRY
        });

        // remove entries, if we are full
        while (self.size + load_size) > MAX_BUFFERED_BLKS {
            // remove oldest entry
            let mut head = self.lru.pop_front().unwrap();

            if head.locked {
                // wait for block to be evicted
                log!(
                    LogFlags::FSBuf,
                    "filebuffer: waiting for eviction of blocks <{:?}>",
                    head.blocks,
                );
                thread::wait_for(head.unlock);
            }
            else {
                // remove from treap
                log!(
                    LogFlags::FSBuf,
                    "filebuffer: evict blocks <{:?}>",
                    head.blocks
                );
                self.entries.remove(&head.blocks);

                // write it back, if it's dirty
                head.flush().unwrap();

                // revoke access from clients
                // TODO currently, clients are not prepared for that
                m3::tiles::Activity::own()
                    .revoke(
                        m3::kif::CapRngDesc::new(m3::kif::CapType::Object, head.data.sel(), 1),
                        false,
                    )
                    .unwrap();

                // we have more space now
                self.size -= head.blocks.count as usize;
            }
        }

        // create new entry (boxed to ensure its pointer stays constant)
        let mut new_head = Box::new(FileBufferEntry::new(
            BlockRange::new_range(bno, load_size as BlockNo),
            self.block_size,
        )?);
        self.size += new_head.blocks.count as usize;

        log!(
            LogFlags::FSBuf,
            "filebuffer: allocated blocks <{:?}>{}",
            new_head.blocks,
            if load.is_some() { " (loading)" } else { "" }
        );

        // load data from backend
        backend.load_data(
            &new_head.data,
            new_head.blocks,
            load.is_some(),
            new_head.unlock,
        )?;
        new_head.locked = false;

        m3::syscalls::derive_mem(
            m3::tiles::Activity::own().sel(),
            sel,
            new_head.data.sel(),
            0,
            (load_size * self.block_size) as GlobOff,
            perm,
        )?;

        new_head.dirty = perm.contains(Perm::W);

        // everything went fine, so insert pointer into treap and the object into the LRU list
        let ptr = unsafe { NonNull::new_unchecked(&mut *new_head as *mut _) };
        self.entries.insert(new_head.blocks, ptr);
        self.lru.push_back(new_head);

        Ok(load_size * self.block_size)
    }

    pub fn flush(&mut self) -> Result<(), Error> {
        while let Some(mut b) = self.lru.pop_front() {
            self.entries.remove(&b.blocks);
            b.flush()?;
        }

        Ok(())
    }
}
