use crate::backend::Backend;
use crate::buffer::{Buffer, PRDT_SIZE};
use crate::internal::BlockNo;
use crate::util::*;

use m3::boxed::Box;
use m3::cap::Selector;
use m3::cell::RefCell;
use m3::col::BoxList;
use m3::col::Treap;
use m3::com::{MemGate, Perm};
use m3::rc::Rc;

use core::ptr::NonNull;
use thread::Event;

pub struct FileBufferHead {
    bno: BlockNo,
    data: m3::com::MemGate,

    lru_entry: Rc<RefCell<LruElement<BlockNo>>>,

    size: usize,
    locked: bool,
    dirty: bool,
    unlock: Event,
}

impl core::cmp::PartialEq for FileBufferHead {
    fn eq(&self, other: &Self) -> bool {
        self.bno == other.bno
    }
}

impl FileBufferHead {
    pub fn new(bno: BlockNo, size: usize, blocksize: usize) -> Self {
        FileBufferHead {
            bno,
            data: MemGate::new(size * blocksize + PRDT_SIZE, Perm::RWX)
                .expect("Failed to create mem gate for FileBufferHead"),

            lru_entry: LruElement::new(bno),

            size,
            locked: true,
            dirty: false,
            unlock: thread::ThreadManager::get().alloc_event(),
        }
    }
}

pub struct FileBuffer {
    size: usize,

    ht: Treap<BlockNo, Rc<RefCell<FileBufferHead>>>,
    //Least recently used list, the front element is least recently used, the last element is the most recently used one.
    lru: Lru<BlockNo>,

    block_size: usize,
}

pub const FILE_BUFFER_SIZE: usize = 16384;

impl FileBuffer {
    pub fn new(block_size: usize) -> Self {
        FileBuffer {
            size: 0, //No size atm

            ht: Treap::new(),
            lru: Lru::new(),

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
        dirty: Option<bool>,
    ) -> usize {
        let load = load.unwrap_or(true);
        let dirty = dirty.unwrap_or(false);

        loop {
            if let Some(head) = self.get(bno).cloned() {
                // Think this is redundant, but the c++ impl uses the key explicitly.
                //TODO Test, might remove
                let key: BlockNo = head.borrow().bno;

                if head.borrow().locked {
                    //Wait for block to unlock
                    log!(
                        crate::LOG_DEF,
                        "FileFuffer: Waiting for cached blocks <{},{}> for block {}",
                        key,
                        head.borrow().size,
                        bno
                    );
                    thread::ThreadManager::get().wait_for(head.borrow().unlock);
                }
                else {
                    self.lru.move_to_back(head.borrow().lru_entry.clone());

                    log!(
                        crate::LOG_DEF,
                        "FileBuffer: Found cached blocks <{},{}> for block {}",
                        key,
                        head.borrow().size,
                        bno
                    );
                    let len = size.min(head.borrow().size - (bno - key) as usize);
                    m3::syscalls::derive_mem(
                        m3::pes::VPE::cur().sel(),
                        sel,
                        head.borrow().data.sel(),
                        ((bno - key) as u64) * self.block_size as u64,
                        len * self.block_size,
                        perm,
                    )
                    .expect("Failed to derive memory for block!");

                    head.borrow_mut().dirty |= dirty;

                    return len * self.block_size;
                }
            }
            else {
                break;
            }
        }

        //There is no block for the given bno.
        //Load chunk into memory
        let max_size: usize = FILE_BUFFER_SIZE.min((1 as usize) << accessed);
        let load_size: usize = size.min(if load { max_size } else { FILE_BUFFER_SIZE });

        if (self.size + load_size) > FILE_BUFFER_SIZE {
            while (self.size + load_size) > FILE_BUFFER_SIZE {
                //Get the key of the first element in the lru chain.
                let key = *self
                    .lru
                    .front()
                    .expect("Could not get front element of lru")
                    .borrow()
                    .value();

                //Get head for key of first element
                if let Some(head) = self.ht.get(&key).cloned() {
                    if head.borrow().locked {
                        //Wait for block to be evicted. We then have more space. Maybe enough space to store the new data
                        log!(
                            crate::LOG_DEF,
                            "FileBuffer: Waiting for eviction of block <{}>",
                            key
                        );
                        thread::ThreadManager::get().wait_for(head.borrow().unlock);
                    }
                    else {
                        //Evict oldest block
                        log!(crate::LOG_DEF, "FileBuffer: Evict block <{}>", key);
                        let _oldest_head_in_lru = self.lru.pop_front();
                        let mut oldest_head_in_ht = self
                            .ht
                            .remove(&key)
                            .expect("Could not delete head when allocating in file buffer!");

                        if head.borrow().dirty {
                            //If the head we are changing is dirty, flush it to the disk before
                            // removing it
                            self.flush_chunk(&mut oldest_head_in_ht);
                        }
                        m3::pes::VPE::cur()
                            .revoke(
                                m3::kif::CapRngDesc::new(
                                    m3::kif::CapType::OBJECT,
                                    head.borrow().data.sel(),
                                    1,
                                ),
                                false,
                            )
                            .expect("Failed to revoke VPE capabilities");
                        //Remove head from inner Buffer size
                        self.size -= head.borrow().size;
                    }
                }
            }
        }

        //At this point there must be enough space for the block
        let new_head = Rc::new(RefCell::new(FileBufferHead::new(
            bno,
            load_size,
            self.block_size as usize,
        )));
        self.size += new_head.borrow().size;
        self.ht.insert(bno, new_head.clone());
        let lru_entry = new_head.borrow().lru_entry.clone();
        self.lru.move_to_back(lru_entry);

        log!(
            crate::LOG_DEF,
            "FileBuffer: Allocating blocks <{},{}> {}",
            new_head.borrow().bno,
            new_head.borrow().size,
            if load { "loading" } else { "" }
        );
        //Load data from backend
        backend.load_data(
            &new_head.borrow().data,
            new_head.borrow().bno,
            new_head.borrow().size,
            load,
            new_head.borrow().unlock,
        );
        new_head.borrow_mut().locked = false;

        m3::syscalls::derive_mem(
            m3::pes::VPE::cur().sel(),
            sel,
            new_head.borrow().data.sel(),
            0,
            load_size * self.block_size,
            perm,
        )
        .expect("Failed to derive memory for file buffer block!");

        new_head.borrow_mut().dirty = dirty;
        return load_size * self.block_size;
    }
}

impl Buffer for FileBuffer {
    type HEAD = Rc<RefCell<FileBufferHead>>;

    fn mark_dirty(&mut self, bno: BlockNo) {
        if let Some(b) = self.get_mut(bno) {
            b.borrow_mut().dirty = true;
        }
    }

    fn flush(&mut self) {
        while !self.ht.is_empty() {
            if let Some(mut head) = self.ht.remove_root() {
                if head.borrow().dirty {
                    self.flush_chunk(&mut head);
                }
            }
            else {
                break;
            }
        }
    }

    fn get(&self, bno: BlockNo) -> Option<&Rc<RefCell<FileBufferHead>>> {
        self.ht.get(&bno)
    }

    fn get_mut(&mut self, bno: BlockNo) -> Option<&mut Rc<RefCell<FileBufferHead>>> {
        self.ht.get_mut(&bno)
    }

    fn flush_chunk(&mut self, head: &Rc<RefCell<FileBufferHead>>) {
        head.borrow_mut().locked = true;
        log!(
            crate::LOG_DEF,
            "FileBuffer: Write back blocks <{},{}>",
            head.borrow().bno,
            head.borrow().size
        );

        //Write data of block to backend
        crate::hdl().backend().store_data(
            head.borrow().bno,
            head.borrow().size,
            head.borrow().unlock,
        );

        //Reset dirty and unlock
        head.borrow_mut().dirty = false;
        head.borrow_mut().locked = false;
    }
}
