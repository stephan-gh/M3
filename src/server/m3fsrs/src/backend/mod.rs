mod disk_backend;
mod mem_backend;

pub use disk_backend::DiskBackend;
pub use mem_backend::MemBackend;

use crate::buf::{LoadLimit, MetaBufferBlock};
use crate::data::{BlockNo, BlockRange, Extent, SuperBlock};

use m3::cap::Selector;
use m3::com::MemGate;
use m3::com::Perm;
use m3::errors::Error;
use thread::Event;

pub trait Backend {
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
        blocks: BlockRange,
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

    fn store_data(&self, blocks: BlockRange, unlock: Event) -> Result<(), Error>;

    fn sync_meta(&self, bno: BlockNo) -> Result<(), Error>;

    fn get_filedata(
        &self,
        ext: Extent,
        extoff: usize,
        perms: Perm,
        sel: Selector,
        load: Option<&mut LoadLimit>,
    ) -> Result<usize, Error>;

    fn clear_extent(&self, ext: Extent) -> Result<(), Error>;

    fn load_sb(&mut self) -> Result<SuperBlock, Error>;

    fn store_sb(&self, super_block: &SuperBlock) -> Result<(), Error>;
}
