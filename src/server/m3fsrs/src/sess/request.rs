use m3::cell::RefCell;
use m3::col::Vec;
use m3::errors::*;
use m3::rc::Rc;

use crate::meta_buffer::MetaBufferHead;

pub struct Request {
    last_error: Option<Code>,
    blocks: Vec<Rc<RefCell<MetaBufferHead>>>,
}

impl Request {
    pub fn new() -> Self {
        Request {
            last_error: None,
            blocks: Vec::new(),
        }
    }

    pub fn has_error(&self) -> bool {
        self.last_error.is_some()
    }

    pub fn error(&self) -> Option<Code> {
        self.last_error
    }

    pub fn set_error(&mut self, err: Code) {
        self.last_error = Some(err);
    }

    pub fn used_meta(&self) -> usize {
        self.blocks.len()
    }

    pub fn push_meta(&mut self, meta: Rc<RefCell<MetaBufferHead>>) {
        self.blocks.push(meta);
    }

    pub fn pop_meta(&mut self) {
        self.blocks.pop().unwrap();
    }

    pub fn pop_metas(&mut self, n: usize) {
        for _i in 0..n {
            self.pop_meta();
        }
    }
}
