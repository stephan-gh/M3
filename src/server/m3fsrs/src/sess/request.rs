use m3::col::Vec;

pub struct Request {
    blocks: Vec<usize>,
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

    pub fn push_meta(&mut self, meta: usize) {
        self.blocks.push(meta);
    }

    pub fn pop_meta(&mut self) {
        crate::hdl().metabuffer().deref(self.blocks.pop().unwrap());
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
