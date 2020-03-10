use crate::backend::{Backend, SuperBlock};
use crate::internal::*;
use crate::meta_buffer::MetaBufferHead;
use crate::sess::request::Request;

use crate::m3::serialize::Sink;
use m3::cap::Selector;
use m3::cell::RefCell;
use m3::com::{MemGate, Perm};
use m3::errors::Error;
use m3::kif::{CapRngDesc, CapType};
use m3::rc::Rc;
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
            blocksize: 4096, //gets initialized when loading superblock
            disk,
            metabuf, //Gets set as well when loading supper block
        })
    }

    fn delegate_mem(&self, mem: &MemGate, bno: BlockNo, len: usize) {
        let crd = CapRngDesc::new(CapType::OBJECT, mem.sel(), 1);
        self.disk
            .sess
            .delegate(
                crd,
                |slice_sink| {
                    //Add arguments in order
                    slice_sink.push(&bno);
                    slice_sink.push(&len);
                },
                |_slice_source| Ok(()),
            )
            .unwrap();
    }
}

impl Backend for DiskBackend {
    fn in_memory(&self) -> bool {
        false
    }

    fn load_meta(
        &self,
        dst: Rc<RefCell<MetaBufferHead>>,
        dst_off: usize,
        bno: BlockNo,
        unlock: Event,
    ) {
        let off = dst_off * (self.blocksize + crate::buffer::PRDT_SIZE);
        self.disk
            .read(0, bno, 1, self.blocksize, Some(off as u64))
            .unwrap();
        self.metabuf
            .read_bytes(
                dst.borrow_mut().data_mut().as_mut_ptr(),
                self.blocksize,
                off as u64,
            )
            .unwrap();
        thread::ThreadManager::get().notify(unlock, None);
    }

    fn load_data(&self, mem: &MemGate, bno: BlockNo, blocks: usize, init: bool, unlock: Event) {
        self.delegate_mem(mem, bno, blocks);
        if init {
            self.disk
                .read(bno, bno, blocks, self.blocksize, None)
                .unwrap();
        }
        thread::ThreadManager::get().notify(unlock, None);
    }

    fn store_meta(
        &self,
        src: Rc<RefCell<MetaBufferHead>>,
        src_off: usize,
        bno: BlockNo,
        unlock: Event,
    ) {
        let off = src_off * (self.blocksize + crate::buffer::PRDT_SIZE);
        self.metabuf
            .write_bytes(
                src.borrow_mut().data_mut().as_mut_ptr(),
                self.blocksize,
                off as u64,
            )
            .unwrap();
        self.disk
            .write(0, bno, 1, self.blocksize, Some(off as u64))
            .unwrap();
        thread::ThreadManager::get().notify(unlock, None);
    }

    fn store_data(&self, bno: BlockNo, blocks: usize, unlock: Event) {
        self.disk
            .write(bno, bno, blocks, self.blocksize, None)
            .unwrap();
        thread::ThreadManager::get().notify(unlock, None);
    }

    fn sync_meta(&self, request: &mut Request, bno: &BlockNo) {
        // check if there is a filebuffer entry for it or create one
        let msel = m3::pes::VPE::cur().alloc_sel();
        let ret = crate::hdl().filebuffer().get_extent(
            self,
            *bno,
            1,
            msel,
            Perm::RWX,
            1,
            Some(false),
            None,
        );
        if ret > 0 {
            // okay, so write it from metabuffer to filebuffer
            let m = MemGate::new_bind(msel);
            let block_borrow = crate::hdl().metabuffer().get_block(request, *bno, false);
            m.write_bytes(
                block_borrow.borrow_mut().data_mut().as_mut_ptr(),
                crate::hdl().superblock().block_size as usize,
                0,
            )
            .unwrap();
            request.pop_meta();
        }
        else {
            // if the filebuffer entry didn't exist and couldn't be created, update block on disk
            crate::hdl().metabuffer().write_back(bno);
        }
    }

    fn get_filedata(
        &self,
        _req: &Request,
        ext: &mut LoadedExtent,
        extoff: usize,
        perms: Perm,
        sel: Selector,
        dirty: bool,
        load: bool,
        accessed: usize,
    ) -> usize {
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

    fn clear_extent(&self, _request: &mut Request, extent: &LoadedExtent, accessed: usize) {
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
            );
            let mem = MemGate::new_bind(sel);
            mem.write_bytes(zeros.as_mut_ptr(), bytes, 0).unwrap();
            i += bytes as u32 / self.blocksize as u32;
        }
    }

    ///Loads a new superblock
    fn load_sb(&mut self) -> SuperBlock {
        let tmp = MemGate::new(512 + crate::buffer::PRDT_SIZE, Perm::RW).unwrap();
        self.delegate_mem(&tmp, 0, 1);
        self.disk
            .read(0, 0, 1, 512, None)
            .expect("Failed to read superblock from disk teddy");
        let sbs: SuperBlockStorage = tmp
            .read_obj::<SuperBlockStorage>(0)
            .expect("Failed to read superblock from disk");
        let super_block = sbs.to_superblock();

        super_block.log();

        // use separate transfer buffer for each entry to allow parallel disk requests
        self.blocksize = super_block.block_size as usize;
        let size =
            (self.blocksize + crate::buffer::PRDT_SIZE) * crate::meta_buffer::META_BUFFER_SIZE;
        self.metabuf = MemGate::new(size, Perm::RW).expect("Failed to create disk transfer buffer");
        // store the MemCap as blockno 0, bc we won't load the superblock again
        self.delegate_mem(&self.metabuf, 0, 1);
        super_block
    }

    fn store_sb(&self, super_block: &SuperBlock) {
        *super_block.checksum.borrow_mut() = super_block.get_checksum();
        self.metabuf
            .write_obj(&super_block.to_storage(), 0)
            .unwrap();
        self.disk.write(0, 0, 1, 512, None).unwrap();
    }
}
