use m3::cell::RefCell;
use m3::col::Vec;
use m3::rc::Rc;

use crate::meta_buffer::MetaBufferHead;

pub struct Request {
    blocks: Vec<Rc<RefCell<MetaBufferHead>>>,
}

impl Request {
    pub fn new() -> Self {
        Request {
            blocks: Vec::new(),
        }
    }

    pub fn used_meta(&self) -> usize {
        self.blocks.len()
    }

    pub fn push_meta(&mut self, meta: Rc<RefCell<MetaBufferHead>>) {
        self.blocks.push(meta);
    }

    pub fn pop_meta(&mut self) {
        // TODO actually dereference the block here
        self.blocks.pop().unwrap();
    }

    pub fn pop_metas(&mut self, n: usize) {
        for _i in 0..n {
            self.pop_meta();
        }
    }
}

impl Drop for Request {
    fn drop(&mut self) {
        self.pop_metas(self.blocks.len());
    }
}
