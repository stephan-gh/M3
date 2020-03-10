use crate::internal::*;

pub const PRDT_SIZE: usize = 8;

///Implemented by File and Meta buffer, defines shared behavior.
pub trait Buffer {
    type HEAD;

    fn mark_dirty(&mut self, bno: BlockNo);
    fn flush(&mut self);

    fn get(&self, bno: BlockNo) -> Option<&Self::HEAD>;
    fn get_mut(&mut self, bno: BlockNo) -> Option<&mut Self::HEAD>;

    fn flush_chunk(&mut self, head: &Self::HEAD);
}
