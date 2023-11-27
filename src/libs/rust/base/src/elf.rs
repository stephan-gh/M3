/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2020 Nils Asmussen, Barkhausen Institut
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

//! Contains the basics of the ELF interface

use bitflags::bitflags;

use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::kif;

const EI_NIDENT: usize = 16;

/// The program header entry types
#[derive(Copy, Clone, Default, Debug, Eq, PartialEq, IntoPrimitive, TryFromPrimitive)]
#[repr(u32)]
pub enum PHType {
    /// Load segment
    #[default]
    Load = 1,
}

bitflags! {
    /// The program header flags
    #[derive(Copy, Clone, Default, Debug, PartialEq, Eq)]
    pub struct PHFlags : u32 {
        /// Executable
        const X = 0x1;
        /// Writable
        const W = 0x2;
        /// Readable
        const R = 0x4;
    }
}

/// ELF header
#[derive(Default, Debug)]
#[repr(C)]
pub struct ElfHeader {
    /// ELF magic: ['\x7F', 'E', 'L', 'F']
    pub ident: [u8; EI_NIDENT],
    /// ELF type (e.g., executable)
    pub ty: u16,
    /// Machine the ELF binary was built for
    pub machine: u16,
    /// ELF version
    pub version: u32,
    /// Entry point of the program
    pub entry: usize,
    /// Program header offset
    pub ph_off: usize,
    /// Section header offset
    pub sh_off: usize,
    /// ELF flags
    pub flags: u32,
    /// Size of the ELF header
    pub eh_size: u16,
    /// Size of program headers
    pub ph_entry_size: u16,
    /// Number of program headers
    pub ph_num: u16,
    /// Size of section headers
    pub sh_entry_size: u16,
    /// Number of section headers
    pub sh_num: u16,
    /// Section header string table index
    pub sh_string_idx: u16,
}
#[cfg(target_pointer_width = "32")]
const _: () = assert!(crate::mem::size_of::<ElfHeader>() == 52);
#[cfg(target_pointer_width = "64")]
const _: () = assert!(crate::mem::size_of::<ElfHeader>() == 64);

/// Program header for 32-bit ELF files
#[derive(Default, Debug)]
#[repr(C)]
#[cfg(target_pointer_width = "32")]
pub struct ProgramHeader32 {
    /// Program header type
    pub ty: u32,
    /// File offset
    pub offset: u32,
    /// Virtual address
    pub virt_addr: usize,
    /// Physical address
    pub phys_addr: usize,
    /// Size of this program header in the file
    pub file_size: u32,
    /// Size of this program header in memory
    pub mem_size: u32,
    /// Program header flags
    pub flags: u32,
    /// Alignment
    pub align: u32,
}
#[cfg(target_pointer_width = "32")]
const _: () = assert!(crate::mem::size_of::<ProgramHeader32>() == 32);

/// Program header for 64-bit ELF files
#[derive(Default, Debug)]
#[repr(C)]
#[cfg(target_pointer_width = "64")]
pub struct ProgramHeader64 {
    /// Program header type
    pub ty: u32,
    /// Program header flags
    pub flags: u32,
    /// File offset
    pub offset: u64,
    /// Virtual address
    pub virt_addr: usize,
    /// Physical address
    pub phys_addr: usize,
    /// Size of this program header in the file
    pub file_size: u64,
    /// Size of this program header in memory
    pub mem_size: u64,
    /// Alignment
    pub align: u64,
}
#[cfg(target_pointer_width = "64")]
const _: () = assert!(crate::mem::size_of::<ProgramHeader64>() == 56);

/// Program header (64-bit)
#[cfg(target_pointer_width = "64")]
pub type ProgramHeader = ProgramHeader64;
/// Program header (32-bit)
#[cfg(target_pointer_width = "32")]
pub type ProgramHeader = ProgramHeader32;

impl From<PHFlags> for kif::Perm {
    fn from(flags: PHFlags) -> Self {
        let mut prot = kif::Perm::empty();
        if flags.contains(PHFlags::R) {
            prot |= kif::Perm::R;
        }
        if flags.contains(PHFlags::W) {
            prot |= kif::Perm::W;
        }
        if flags.contains(PHFlags::X) {
            prot |= kif::Perm::X;
        }
        prot
    }
}
