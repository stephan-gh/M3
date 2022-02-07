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

use crate::data::{BlockNo, NUM_EXT_BYTES, NUM_INODE_BYTES};

/// Represents a superblock
#[derive(Debug)]
#[repr(C, align(8))]
pub struct SuperBlock {
    pub block_size: u32,
    pub total_inodes: u32,
    pub total_blocks: u32,
    pub free_inodes: u32,
    pub free_blocks: u32,
    pub first_free_inode: u32,
    pub first_free_block: u32,
    pub checksum: u32,
}

impl SuperBlock {
    pub fn get_checksum(&self) -> u32 {
        1 + self.block_size * 2
            + self.total_inodes * 3
            + self.total_blocks * 5
            + self.free_inodes * 7
            + self.free_blocks * 11
            + self.first_free_inode * 13
            + self.first_free_block * 17
    }

    pub fn first_inodebm_block(&self) -> BlockNo {
        1
    }

    pub fn inodebm_block(&self) -> BlockNo {
        (((self.total_inodes + 7) / 8) + self.block_size - 1) / self.block_size
    }

    pub fn first_blockbm_block(&self) -> BlockNo {
        self.first_inodebm_block() + self.inodebm_block()
    }

    pub fn blockbm_blocks(&self) -> BlockNo {
        (((self.total_blocks + 7) / 8) + self.block_size - 1) / self.block_size
    }

    pub fn first_inode_block(&self) -> BlockNo {
        self.first_blockbm_block() + self.blockbm_blocks()
    }

    pub fn extents_per_block(&self) -> usize {
        self.block_size as usize / NUM_EXT_BYTES
    }

    pub fn inodes_per_block(&self) -> usize {
        self.block_size as usize / NUM_INODE_BYTES
    }

    pub fn update_inodebm(&mut self, free: u32, first: u32) {
        self.free_inodes = free;
        self.first_free_inode = first;
    }

    pub fn update_blockbm(&mut self, free: u32, first: u32) {
        self.free_blocks = free;
        self.first_free_block = first;
    }
}
