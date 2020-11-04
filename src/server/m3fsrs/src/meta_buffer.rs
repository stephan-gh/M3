use crate::buffer::Buffer;
use crate::internal::*;
use crate::sess::request::Request;
use crate::util::*;

use m3::boxed::Box;
use m3::cell::RefCell;
use m3::col::BoxList;
use m3::col::Treap;
use m3::col::Vec;
use m3::rc::Rc;

use core::ptr::NonNull;
use thread::Event;

pub struct LRUEntry {
    head: Option<Rc<RefCell<MetaBufferHead>>>,
}

/// The BufferHead represents a single block within the MetaBuffer. In Rust, contrary to C++ it also
/// handles runtime type checking. When loading an Inode, extent, or DirEntry from it, it checks if this entity has already been loaded.
/// If that's the case, a reference to this entity is returned, otherwise a new entity is loaded and returned.
pub struct MetaBufferHead {
    bno: BlockNo,

    lru_entry: Rc<RefCell<LruElement<LRUEntry>>>,

    locked: bool,
    dirty: bool,
    unlock: Event,

    off: usize,
    data: Vec<u8>,
}

impl core::cmp::PartialEq for MetaBufferHead {
    fn eq(&self, other: &Self) -> bool {
        self.bno == other.bno
    }
}

pub const META_BUFFER_SIZE: usize = 128;

impl MetaBufferHead {
    pub fn new(
        bno: BlockNo,
        off: usize,
        blocksize: usize,
        lru_entry: Rc<RefCell<LruElement<LRUEntry>>>,
    ) -> Self {
        MetaBufferHead {
            bno,

            lru_entry,

            locked: true,
            dirty: false,
            unlock: thread::ThreadManager::get().alloc_event(),

            off,
            data: vec![0; blocksize as usize],
        }
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    //Overwrites the data of this block with zeros
    pub fn overwrite_zero(&mut self) {
        //we are allowed to move the memory loction, since the overwrite invalidates the whole
        //block anyways
        self.data = vec![0; crate::hdl().superblock().block_size as usize];
    }
}

pub struct MetaBuffer {
    ht: Treap<BlockNo, Rc<RefCell<MetaBufferHead>>>,
    lru: Lru<LRUEntry>,
}

impl MetaBuffer {
    pub fn new(blocksize: usize) -> Self {
        let mut lru = Lru::new();
        for i in 0..META_BUFFER_SIZE {
            let entry = LruElement::new(LRUEntry { head: None });

            let meta_buffer_head = Rc::new(RefCell::new(MetaBufferHead::new(
                0,
                i,
                blocksize,
                entry.clone(),
            )));

            entry.borrow_mut().value_mut().head = Some(meta_buffer_head.clone());

            lru.push_back(entry) //Init a block for each slot
        }

        MetaBuffer {
            ht: Treap::new(),
            lru,
        }
    }

    ///Searches for data at `bno`, allocates if none is present.
    pub fn get_block(
        &mut self,
        req: &mut Request,
        bno: BlockNo,
        dirty: bool,
    ) -> Rc<RefCell<MetaBufferHead>> {
        log!(
            crate::LOG_DEF,
            "MetaBuffer::get_block(bno={}, dirty={})",
            bno,
            dirty
        );
        loop {
            if let Some(head) = self.get(bno) {
                let head = head.clone();

                if head.borrow().locked {
                    //TODO find out why this breaks the linker
                    //panic!("Could not wait for locked block, linking is broken atm.");
                    //log!(crate::LOG_DEF, "WARNING: No really waiting for unlock: TODO find out why this breaks the linker!");
                    //thread::ThreadManager::get().wait_for(head.borrow().unlock);
                    //head.borrow_mut().locked = false;
                    self.flush_chunk(&head);
                }
                else {
                    //Move element to back since it was touched
                    let lru_entry = head.borrow().lru_entry.clone();
                    self.lru.move_to_back(lru_entry);
                    head.borrow_mut().dirty |= dirty;
                    log!(
                        crate::LOG_DEF,
                        "MetaBuffer: Found cached block <{}>, Links: {}",
                        head.borrow().bno,
                        Rc::strong_count(&head)
                    );
                    req.push_meta(head.clone());
                    return head;
                }
            }
            else {
                //No block for block number, therefore allocate
                break;
            }
        }

        let mut use_block = None;
        //Find first unused head
        for lru_element in self.lru.iter() {
            if Rc::strong_count(lru_element.borrow().value().head.as_ref().unwrap()) <= 2 {
                //Only saved in lru and ht but unused
                use_block = Some(lru_element.borrow().value().head.as_ref().unwrap().clone());
                break;
            }
        }

        let block: Rc<RefCell<MetaBufferHead>> = use_block.unwrap();

        //Flush if there is still a block present with the given bno.
        if let Some(mut old_block) = self.ht.remove(&block.borrow().bno) {
            if old_block.borrow().dirty {
                self.flush_chunk(&mut old_block);
            }
        }

        //Now we are save to use this bno
        //Insert into ht
        block.borrow_mut().bno = bno;
        self.ht.insert(bno, block.clone());

        let off = block.borrow().off;
        let unlock = block.borrow().unlock;
        //Now load from backend and setup everything
        crate::hdl()
            .backend()
            .load_meta(block.clone(), off, bno, unlock);
        block.borrow_mut().dirty = dirty;
        let lru_element = block.borrow().lru_entry.clone();
        self.lru.move_to_back(lru_element);

        log!(
            crate::LOG_DEF,
            "MetaBuffer: Load new block<{}> Links: {}",
            bno,
            Rc::strong_count(&block)
        );
        block.borrow_mut().locked = false;

        req.push_meta(block.clone());
        block.clone()
    }

    pub fn dirty(&self, bno: BlockNo) -> bool {
        if let Some(b) = self.get(bno) {
            b.borrow().dirty
        }
        else {
            false
        }
    }

    pub fn write_back(&mut self, bno: &BlockNo) {
        if let Some(h) = self.get(*bno) {
            let inner = h.clone();
            if inner.borrow().dirty {
                self.flush_chunk(&inner);
            }
        }
    }
}

impl Buffer for MetaBuffer {
    type HEAD = Rc<RefCell<MetaBufferHead>>;

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

    fn get(&self, bno: BlockNo) -> Option<&Self::HEAD> {
        self.ht.get(&bno)
    }

    fn get_mut(&mut self, bno: BlockNo) -> Option<&mut Self::HEAD> {
        self.ht.get_mut(&bno)
    }

    fn flush_chunk(&mut self, head: &Self::HEAD) {
        head.borrow_mut().locked = true;
        log!(
            crate::LOG_DEF,
            "MetaBuffer: Write back block <{}>",
            head.borrow().bno
        );

        //Write meta block to backend device
        crate::hdl().backend().store_meta(
            head.clone(),
            head.borrow().off,
            head.borrow().bno,
            head.borrow().unlock,
        );
        head.borrow_mut().dirty = false;
        head.borrow_mut().locked = false;
    }
}
