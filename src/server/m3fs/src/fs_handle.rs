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

use crate::buf::MetaBuffer;

use m3::errors::Error;

/// Handle with global data structures needed at various places
pub struct M3FSHandle {
    meta_buffer: MetaBuffer,
}

impl M3FSHandle {
    pub fn new() -> Self {
        let sb = crate::superblock();

        M3FSHandle {
            meta_buffer: MetaBuffer::new(sb.block_size as usize),
        }
    }

    pub fn flush_buffer(&mut self) -> Result<(), Error> {
        self.meta_buffer.flush()?;
        crate::file_buffer_mut().flush()?;

        // update superblock and write it back to disk/memory
        let mut sb = crate::superblock_mut();
        let inodes = crate::inodes_mut();
        sb.update_inodebm(inodes.free_count(), inodes.first_free());
        let blocks = crate::blocks_mut();
        sb.update_blockbm(blocks.free_count(), blocks.first_free());
        sb.checksum = sb.get_checksum();
        crate::backend_mut().store_sb(&*sb)
    }

    pub fn metabuffer(&mut self) -> &mut MetaBuffer {
        &mut self.meta_buffer
    }
}
