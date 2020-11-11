/*
 * Copyright (C) 2015-2020, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Copyright (C) 2019-2020, Tendsin Mende <tendsin@protonmail.com>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * This file is part of M3 (Microkernel-based SysteM for Heterogeneous Manycores).
 *
 * M3 is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License version 2 as
 * published by the Free Software Foundation.
 *
 * M3 is distributed in the hope that it will be useful, but
 * WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * General Public License version 2 for more details.
 */

use crate::backend::{Backend, SuperBlock};
use crate::buf::{LoadLimit, MetaBufferBlock};
use crate::data::{BlockNo, BlockRange, Extent};

use m3::cap::Selector;
use m3::com::{MemGate, Perm};
use m3::errors::Error;
use m3::kif::INVALID_SEL;
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

        Ok(DiskBackend {
            blocksize: 0, // gets initialized when loading superblock
            disk,
            metabuf: MemGate::new_bind(INVALID_SEL), // gets replaced when loading superblock
        })
    }
}

impl Backend for DiskBackend {
    fn load_meta(
        &self,
        dst: &mut MetaBufferBlock,
        dst_off: usize,
        bno: BlockNo,
        unlock: Event,
    ) -> Result<(), Error> {
        let off = dst_off * (self.blocksize + crate::buf::PRDT_SIZE);
        self.disk
            .read(0, BlockRange::new(bno), self.blocksize, Some(off as u64))?;
        self.metabuf
            .read_bytes(dst.data_mut().as_mut_ptr(), self.blocksize, off as u64)?;
        thread::ThreadManager::get().notify(unlock, None);
        Ok(())
    }

    fn load_data(
        &self,
        mem: &MemGate,
        blocks: BlockRange,
        init: bool,
        unlock: Event,
    ) -> Result<(), Error> {
        self.disk.delegate_mem(mem, blocks)?;
        if init {
            self.disk.read(blocks.start, blocks, self.blocksize, None)?;
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
        let off = src_off * (self.blocksize + crate::buf::PRDT_SIZE);
        self.metabuf
            .write_bytes(src.data().as_ptr(), self.blocksize, off as u64)?;
        self.disk
            .write(0, BlockRange::new(bno), self.blocksize, Some(off as u64))?;
        thread::ThreadManager::get().notify(unlock, None);
        Ok(())
    }

    fn store_data(&self, blocks: BlockRange, unlock: Event) -> Result<(), Error> {
        self.disk.write(blocks.start, blocks, self.blocksize, None)?;
        thread::ThreadManager::get().notify(unlock, None);
        Ok(())
    }

    fn sync_meta(&self, bno: BlockNo) -> Result<(), Error> {
        // check if there is a filebuffer entry for it or create one
        let msel = m3::pes::VPE::cur().alloc_sel();
        crate::hdl()
            .filebuffer()
            .get_extent(self, bno, 1, msel, Perm::RWX, None)?;

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
        ext: Extent,
        extoff: usize,
        perms: Perm,
        sel: Selector,
        load: Option<&mut LoadLimit>,
    ) -> Result<usize, Error> {
        let first_block = extoff / self.blocksize;
        crate::hdl().filebuffer().get_extent(
            self,
            ext.start + first_block as u32,
            ext.length as usize - first_block,
            sel,
            perms,
            load,
        )
    }

    fn clear_extent(&self, ext: Extent) -> Result<(), Error> {
        let mut zeros = [0; crate::data::MAX_BLOCK_SIZE as usize];
        let sel = m3::pes::VPE::cur().alloc_sel();
        let mut i = 0;
        while i < ext.length {
            let bytes = crate::hdl().filebuffer().get_extent(
                self,
                ext.start + i,
                (ext.length - i) as usize,
                sel,
                Perm::RW,
                None,
            )?;
            let mem = MemGate::new_bind(sel);
            mem.write_bytes(zeros.as_mut_ptr(), bytes, 0)?;
            i += bytes as u32 / self.blocksize as u32;
        }
        Ok(())
    }

    fn load_sb(&mut self) -> Result<SuperBlock, Error> {
        let tmp = MemGate::new(512 + crate::buf::PRDT_SIZE, Perm::RW)?;
        self.disk.delegate_mem(&tmp, BlockRange::new(0))?;
        self.disk.read(0, BlockRange::new(0), 512, None)?;
        let super_block = tmp.read_obj::<SuperBlock>(0)?;

        // use separate transfer buffer for each entry to allow parallel disk requests
        self.blocksize = super_block.block_size as usize;
        let size = (self.blocksize + crate::buf::PRDT_SIZE) * crate::buf::META_BUFFER_SIZE;
        self.metabuf = MemGate::new(size, Perm::RW)?;
        // store the MemCap as blockno 0, bc we won't load the superblock again
        self.disk.delegate_mem(&self.metabuf, BlockRange::new(0))?;
        Ok(super_block)
    }

    fn store_sb(&self, super_block: &SuperBlock) -> Result<(), Error> {
        self.metabuf.write_obj(super_block, 0)?;
        self.disk.write(0, BlockRange::new(0), 512, None)
    }
}
