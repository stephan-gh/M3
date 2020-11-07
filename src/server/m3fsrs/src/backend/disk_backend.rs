use crate::backend::{Backend, SuperBlock};
use crate::internal::*;
use crate::meta_buffer::MetaBufferBlock;

use crate::m3::serialize::Sink;
use m3::cap::Selector;
use m3::com::{MemGate, Perm};
use m3::errors::Error;
use m3::kif::{CapRngDesc, CapType};
use m3::session::Disk;
use thread::Event;

pub struct DiskBackend {
    blocksize: usize,
    disk: Disk,
    metabuf: MemGate,
}

impl DiskBackend {
    pub fn new() -> Result<Self, Error> {
        let disk = Disk::new("disk")?;
        let metabuf = MemGate::new(1, Perm::R)?;

        Ok(DiskBackend {
            blocksize: 4096, // gets initialized when loading superblock
            disk,
            metabuf, // Gets set as well when loading supper block
        })
    }

    fn delegate_mem(&self, mem: &MemGate, bno: BlockNo, len: usize) -> Result<(), Error> {
        let crd = CapRngDesc::new(CapType::OBJECT, mem.sel(), 1);
        self.disk.sess.delegate(
            crd,
            |slice_sink| {
                // Add arguments in order
                slice_sink.push(&bno);
                slice_sink.push(&len);
            },
            |_slice_source| Ok(()),
        )
    }
}

impl Backend for DiskBackend {
    fn in_memory(&self) -> bool {
        false
    }

    fn load_meta(
        &self,
        dst: &mut MetaBufferBlock,
        dst_off: usize,
        bno: BlockNo,
        unlock: Event,
    ) -> Result<(), Error> {
        let off = dst_off * (self.blocksize + crate::buffer::PRDT_SIZE);
        self.disk
            .read(0, bno, 1, self.blocksize, Some(off as u64))?;
        self.metabuf
            .read_bytes(dst.data_mut().as_mut_ptr(), self.blocksize, off as u64)?;
        thread::ThreadManager::get().notify(unlock, None);
        Ok(())
    }

    fn load_data(
        &self,
        mem: &MemGate,
        bno: BlockNo,
        blocks: usize,
        init: bool,
        unlock: Event,
    ) -> Result<(), Error> {
        self.delegate_mem(mem, bno, blocks)?;
        if init {
            self.disk.read(bno, bno, blocks, self.blocksize, None)?;
        }
        thread::ThreadManager::get().notify(unlock, None);
        Ok(())
    }

    fn store_meta(
        &self,
        src: &MetaBufferBlock,
        src_off: usize,
        bno: BlockNo,
        unlock: Event,
    ) -> Result<(), Error> {
        let off = src_off * (self.blocksize + crate::buffer::PRDT_SIZE);
        self.metabuf
            .write_bytes(src.data().as_ptr(), self.blocksize, off as u64)?;
        self.disk
            .write(0, bno, 1, self.blocksize, Some(off as u64))?;
        thread::ThreadManager::get().notify(unlock, None);
        Ok(())
    }

    fn store_data(&self, bno: BlockNo, blocks: usize, unlock: Event) -> Result<(), Error> {
        self.disk.write(bno, bno, blocks, self.blocksize, None)?;
        thread::ThreadManager::get().notify(unlock, None);
        Ok(())
    }

    fn sync_meta(&self, bno: BlockNo) -> Result<(), Error> {
        // check if there is a filebuffer entry for it or create one
        let msel = m3::pes::VPE::cur().alloc_sel();
        crate::hdl().filebuffer().get_extent(
            self,
            bno,
            1,
            msel,
            Perm::RWX,
            1,
            Some(false),
            None,
        )?;

        // okay, so write it from metabuffer to filebuffer
        let m = MemGate::new_bind(msel);
        let mut block = crate::hdl().metabuffer().get_block(bno, false)?;
        m.write_bytes(
            block.data_mut().as_mut_ptr(),
            crate::hdl().superblock().block_size as usize,
            0,
        )?;
        Ok(())
    }

    fn get_filedata(
        &self,
        ext: &mut LoadedExtent,
        extoff: usize,
        perms: Perm,
        sel: Selector,
        dirty: bool,
        load: bool,
        accessed: usize,
    ) -> Result<usize, Error> {
        let first_block = extoff / self.blocksize;
        crate::hdl().filebuffer().get_extent(
            self,
            *ext.start() + first_block as u32,
            *ext.length() as usize - first_block,
            sel,
            perms,
            accessed,
            Some(load),
            Some(dirty),
        )
    }

    fn clear_extent(&self, extent: &LoadedExtent, accessed: usize) -> Result<(), Error> {
        let mut zeros: [u8; crate::internal::MAX_BLOCK_SIZE as usize] =
            [0; crate::internal::MAX_BLOCK_SIZE as usize];
        let sel = m3::pes::VPE::cur().alloc_sel();
        let mut i = 0;
        while i < *extent.length() {
            let bytes = crate::hdl().filebuffer().get_extent(
                self,
                *extent.start() + i,
                (*extent.length() - i) as usize,
                sel,
                Perm::RW,
                accessed,
                Some(false),
                Some(true),
            )?;
            let mem = MemGate::new_bind(sel);
            mem.write_bytes(zeros.as_mut_ptr(), bytes, 0)?;
            i += bytes as u32 / self.blocksize as u32;
        }
        Ok(())
    }

    /// Loads a new superblock
    fn load_sb(&mut self) -> Result<SuperBlock, Error> {
        let tmp = MemGate::new(512 + crate::buffer::PRDT_SIZE, Perm::RW)?;
        self.delegate_mem(&tmp, 0, 1)?;
        self.disk.read(0, 0, 1, 512, None)?;
        let sbs: SuperBlockStorage = tmp.read_obj::<SuperBlockStorage>(0)?;
        let super_block = sbs.to_superblock();

        super_block.log();

        // use separate transfer buffer for each entry to allow parallel disk requests
        self.blocksize = super_block.block_size as usize;
        let size =
            (self.blocksize + crate::buffer::PRDT_SIZE) * crate::meta_buffer::META_BUFFER_SIZE;
        self.metabuf = MemGate::new(size, Perm::RW)?;
        // store the MemCap as blockno 0, bc we won't load the superblock again
        self.delegate_mem(&self.metabuf, 0, 1)?;
        Ok(super_block)
    }

    fn store_sb(&self, super_block: &SuperBlock) -> Result<(), Error> {
        *super_block.checksum.borrow_mut() = super_block.get_checksum();
        self.metabuf.write_obj(&super_block.to_storage(), 0)?;
        self.disk.write(0, 0, 1, 512, None)
    }
}
