/*
 * Copyright (C) 2020 Nils Asmussen, Barkhausen Institut
 * Copyright (C) 2019-2020, Tendsin Mende <tendsin@protonmail.com>
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

mod disk_backend;
mod mem_backend;

pub use disk_backend::{DiskBackend, PRDT_SIZE};
pub use mem_backend::MemBackend;

use crate::buf::{LoadLimit, MetaBufferBlock};
use crate::data::{BlockNo, BlockRange, Extent, SuperBlock};

use m3::cap::Selector;
use m3::com::MemCap;
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
        mem: &MemCap,
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

    fn sync_meta(&self, block: &mut MetaBufferBlock) -> Result<(), Error>;

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
