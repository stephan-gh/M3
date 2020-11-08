mod disk_backend;
mod mem_backend;

pub use disk_backend::DiskBackend;
pub use mem_backend::MemBackend;

use crate::internal::Extent;
use crate::meta_buffer::MetaBufferBlock;
use crate::{BlockNo, SuperBlock};

use m3::cap::Selector;
use m3::com::MemGate;
use m3::com::Perm;
use m3::errors::Error;
use thread::Event;

pub trait Backend {
    // Needed for the hotfix. Might be removed.
    fn in_memory(&self) -> bool;

    fn load_meta(
        &self,
        dst: &mut MetaBufferBlock,
        dst_off: usize,
        bno: BlockNo,
        unlock: Event,
    ) -> Result<(), Error>;

    fn load_data(
        &self,
        mem: &MemGate,
        bno: BlockNo,
        blocks: usize,
        init: bool,
        unlock: Event,
    ) -> Result<(), Error>;

    fn store_meta(
        &self,
        src: &MetaBufferBlock,
        src_off: usize,
        bno: BlockNo,
        unlock: Event,
    ) -> Result<(), Error>;

    fn store_data(&self, bno: BlockNo, blocks: usize, unlock: Event) -> Result<(), Error>;

    fn sync_meta(&self, bno: BlockNo) -> Result<(), Error>;

    fn get_filedata(
        &self,
        ext: Extent,
        extoff: usize,
        perms: Perm,
        sel: Selector,
        dirty: bool,
        load: bool,
        accessed: usize,
    ) -> Result<usize, Error>;

    fn clear_blocks(&self, start: BlockNo, count: BlockNo, accessed: usize) -> Result<(), Error>;

    fn load_sb(&mut self) -> Result<SuperBlock, Error>;

    fn store_sb(&self, super_block: &SuperBlock) -> Result<(), Error>;
}
