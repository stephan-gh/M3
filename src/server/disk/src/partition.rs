/*
 * Copyright (C) 2020 Nils Asmussen, Barkhausen Institut
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

use core::fmt;

use m3::col::Vec;

#[derive(Clone, Copy)]
pub struct Partition {
    id: usize,
    present: bool,
    start: u32,
    size: u32,
}

impl Partition {
    pub fn new_whole_disk(size: u32) -> Self {
        Partition {
            id: 0,
            present: true,
            start: 0,
            size,
        }
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn present(&self) -> bool {
        self.present
    }

    pub fn start_sector(&self) -> u32 {
        self.start
    }

    pub fn sector_count(&self) -> u32 {
        self.size
    }
}

impl fmt::Debug for Partition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "Partition[sectors={}..{}]",
            self.start,
            self.start + self.size - 1
        )
    }
}

// offset of partition-table in MBR
const PART_TABLE_OFFSET: isize = 0x1BE;

pub const PART_COUNT: usize = 4;

#[repr(C, packed)]
struct DiskParition {
    // boot indicator bit flag: 0 = no, 0x80 = bootable (or "active")
    bootable: u8,
    // start: Cylinder, Head, Sector
    start_head: u8,
    start_sec_cylinder: u16,
    system_id: u8,
    // end: Cylinder, Head, Sector
    end_head: u8,
    end_sec_cylinder: u16,
    // relative Sector (to start of partition -- also equals the partition's starting LBA value)
    start: u32,
    // total Sectors in partition
    size: u32,
}

pub fn parse_partitions(mbr: &[u8]) -> Vec<Partition> {
    let mut parts = Vec::with_capacity(PART_COUNT);
    let mut src = unsafe { mbr.as_ptr().offset(PART_TABLE_OFFSET) as *const DiskParition };
    for i in 0..PART_COUNT {
        unsafe {
            parts.push(Partition {
                id: i,
                present: (*src).system_id != 0,
                start: (*src).start,
                size: (*src).size,
            });
            src = src.add(1);
        }
    }
    parts
}
