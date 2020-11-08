use crate::backend::{Backend, SuperBlock};
use crate::internal::LoadedExtent;
use crate::meta_buffer::MetaBufferBlock;
use crate::BlockNo;

use m3::cap::Selector;
use m3::com::{MGateArgs, MemGate, Perm};
use m3::errors::Error;
use m3::goff;
use m3::syscalls::derive_mem;
use thread::Event;

pub struct MemBackend {
    mem: MemGate,
    blocksize: usize,
}

impl MemBackend {
    pub fn new(fsoff: goff, fssize: usize) -> Self {
        MemBackend {
            mem: MemGate::new_with(MGateArgs::new(fssize, Perm::RWX).addr(fsoff))
                .expect("Could not create MemGate for Memory Backend"),
            blocksize: 4096, // Gets set when the superblock is read first as well
        }
    }
}

impl Backend for MemBackend {
    fn in_memory(&self) -> bool {
        true
    }

    fn load_meta(
        &self,
        dst: &mut MetaBufferBlock,
        _dst_off: usize,
        bno: BlockNo,
        _unlock: Event,
    ) -> Result<(), Error> {
        self.mem.read_bytes(
            dst.data_mut().as_mut_ptr(),
            self.blocksize,
            (bno as usize * self.blocksize) as u64,
        )
    }

    fn load_data(
        &self,
        _mem: &MemGate,
        _bno: BlockNo,
        _blocks: usize,
        _init: bool,
        _unlock: Event,
    ) -> Result<(), Error> {
        // Unused
        Ok(())
    }

    fn store_meta(
        &self,
        src: &MetaBufferBlock,
        _src_off: usize,
        bno: BlockNo,
        _unlock: Event,
    ) -> Result<(), Error> {
        let slice: &[u8] = src.data();

        self.mem.write(slice, bno as u64 * self.blocksize as u64)
    }

    fn store_data(&self, _bno: BlockNo, _blocks: usize, _unlock: Event) -> Result<(), Error> {
        // unused
        Ok(())
    }

    fn sync_meta(&self, bno: BlockNo) -> Result<(), Error> {
        crate::hdl().metabuffer().write_back(bno)
    }

    fn get_filedata(
        &self,
        ext: &mut LoadedExtent,
        extoff: usize,
        perms: Perm,
        sel: Selector,
        _dirty: bool,
        _load: bool,
        _accessed: usize,
    ) -> Result<usize, Error> {
        let first_block = extoff / self.blocksize;
        let bytes: usize = (ext.length() as usize - first_block) * self.blocksize as usize;
        let size = ((ext.start() as usize + first_block) * self.blocksize) as u64;
        derive_mem(
            m3::pes::VPE::cur().sel(),
            sel,
            self.mem.sel(),
            size,
            bytes,
            perms,
        )?;
        Ok(bytes)
    }

    fn clear_extent(&self, extent: &LoadedExtent, _accessed: usize) -> Result<(), Error> {
        let zeros = vec![0; self.blocksize];
        for block in extent.clone().into_iter() {
            self.mem
                .write(&zeros, (block as usize * self.blocksize) as u64)?;
        }
        Ok(())
    }

    /// Loads a new superblock
    fn load_sb(&mut self) -> Result<SuperBlock, Error> {
        let block = self.mem.read_obj::<SuperBlock>(0)?;
        self.blocksize = block.block_size as usize;
        log!(
            crate::LOG_DEF,
            "MemBackend: Using block_size={}",
            self.blocksize
        );
        Ok(block)
    }

    fn store_sb(&self, super_block: &SuperBlock) -> Result<(), Error> {
        self.mem.write_obj(super_block, 0)
    }
}
