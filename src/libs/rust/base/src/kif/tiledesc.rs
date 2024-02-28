/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

use bitflags::bitflags;
use core::fmt;

use num_enum::{FromPrimitive, IntoPrimitive};

use crate::cfg;
use crate::mem::VirtAddr;
use crate::serialize::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, Eq, PartialEq, IntoPrimitive, FromPrimitive)]
#[repr(u64)]
/// The different types of tiles
pub enum TileType {
    /// Compute tile
    #[default]
    Comp,
    /// Memory tile
    Mem,
}

/// The supported instruction set architectures (ISAs)
#[derive(Copy, Clone, Debug, Eq, PartialEq, IntoPrimitive, FromPrimitive)]
#[repr(u64)]
pub enum TileISA {
    /// Dummy ISA to represent memory tiles
    #[default]
    None,
    /// RISCV as supported on hw and gem5
    RISCV,
    /// x86_64 as supported by gem5
    X86,
    /// ARMv7 as supported by gem5
    ARM,
    /// Dummy ISA to represent the indirect-chaining fixed-function accelerator
    AccelIndir,
    /// Dummy ISA to represent the COPY fixed-function accelerator
    AccelCopy,
    /// Dummy ISA to represent the ROT-13 fixed-function accelerator
    AccelRot13,
    /// Dummy ISA to represent the IDE controller
    IDEDev,
    /// Dummy ISA to represent the NIC
    NICDev,
    /// Dummy ISA to represent the serial input device
    SerialDev,
    /// Dummy ISA to represent a flash with the Common Flash Interface (CFI)
    CFIDev,
}

bitflags! {
    /// The attributes for tiles
    ///
    /// These attributes cover smaller differences between tiles and allow us to choose between
    /// otherwise identical tiles.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct TileAttr : TileDescRaw {
        /// Contains a performance core
        const PERF          = 1 << 0;
        /// Contains an efficiency core
        const EFFI          = 1 << 1;
        /// Contains a NIC
        const NIC           = 1 << 2;
        /// Contains a serial line
        const SERIAL        = 1 << 3;
        /// Contains internal memory (for memory tiles or SPM in compute tiles)
        const IMEM          = 1 << 4;
        /// Contains internal endpoints
        const IEPS          = 1 << 5;
        /// Contains a Keccak Accelerator (KecAcc)
        const KECACC        = 1 << 6;
        /// Root of Trust
        const ROT           = 1 << 7;
    }
}

/// The underlying type of [`TileDesc`]
///
/// +---------------------------+------------+-----+------+
/// | memory size (in 4K pages) | attributes | ISA | type |
/// +---------------------------+------------+-----+------+
/// 64                         28           11     6      0
pub type TileDescRaw = u64;

/// Describes a tile.
///
/// This struct is used for the [`create_activity`] syscall to let the kernel know about the desired tile
/// type. Additionally, it is used to tell a activity about the attributes of the tile it has been assigned
/// to.
///
/// [`create_activity`]: ../../m3/syscalls/fn.create_activity.html
#[repr(C)]
#[derive(Clone, Copy, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TileDesc {
    val: TileDescRaw,
}

impl TileDesc {
    /// Creates a new tile description from the given type, ISA, and memory size.
    pub const fn new(ty: TileType, isa: TileISA, memsize: usize) -> TileDesc {
        let mem_pages = memsize >> 12;
        let val = ty as TileDescRaw | (isa as TileDescRaw) << 6 | (mem_pages as TileDescRaw) << 28;
        Self::new_from(val)
    }

    /// Creates a new tile description from the given type, ISA, memory size, and attributes.
    pub const fn new_with_attr(
        ty: TileType,
        isa: TileISA,
        memsize: usize,
        attr: TileAttr,
    ) -> TileDesc {
        let mem_pages = memsize >> 12;
        let val = ty as TileDescRaw
            | (isa as TileDescRaw) << 6
            | (attr.bits() << 11)
            | (mem_pages as TileDescRaw) << 28;
        Self::new_from(val)
    }

    /// Creates a new tile description from the given raw value
    pub const fn new_from(val: TileDescRaw) -> TileDesc {
        TileDesc { val }
    }

    /// Returns the raw value
    pub fn value(self) -> TileDescRaw {
        self.val
    }

    pub fn tile_type(self) -> TileType {
        TileType::from(self.val & 0x3F)
    }

    pub fn isa(self) -> TileISA {
        TileISA::from((self.val >> 6) & 0x1F)
    }

    pub fn attr(self) -> TileAttr {
        TileAttr::from_bits_truncate((self.val >> 11) & 0x1FFFF)
    }

    /// Returns the size of the internal memory (0 if none is present)
    pub fn mem_size(self) -> usize {
        ((self.val >> 28) as usize) << 12
    }

    /// Returns whether the tile executes software
    pub fn is_programmable(self) -> bool {
        matches!(self.isa(), TileISA::X86 | TileISA::ARM | TileISA::RISCV)
    }

    /// Return if the tile supports multiple contexts
    pub fn is_device(self) -> bool {
        self.isa() == TileISA::NICDev
            || self.isa() == TileISA::IDEDev
            || self.isa() == TileISA::SerialDev
            || self.isa() == TileISA::CFIDev
    }

    /// Return if the tile supports activities
    pub fn supports_activities(self) -> bool {
        self.tile_type() != TileType::Mem
    }

    /// Return if the tile supports the context switching protocol
    pub fn supports_tilemux(self) -> bool {
        self.supports_activities() && !self.is_device()
    }

    /// Returns whether the tile has an internal memory (SPM, DRAM, ...)
    pub fn has_memory(self) -> bool {
        self.tile_type() == TileType::Mem || self.attr().contains(TileAttr::IMEM)
    }

    /// Returns whether the tile has internal endpoints
    pub fn has_internal_eps(self) -> bool {
        self.attr().contains(TileAttr::IEPS)
    }

    /// Returns whether the tile supports virtual memory
    pub fn has_virtmem(self) -> bool {
        // all non-device tiles without internal memory have currently VM support
        !self.has_memory() && !self.is_device()
    }

    /// Derives a new TileDesc from this by changing it based on the given properties.
    pub fn with_properties(&self, props: &str) -> TileDesc {
        let mut res = *self;
        for prop in props.split('+') {
            match prop {
                "arm" => res = TileDesc::new(TileType::Comp, TileISA::ARM, 0),
                "x86" => res = TileDesc::new(TileType::Comp, TileISA::X86, 0),
                "riscv" => res = TileDesc::new(TileType::Comp, TileISA::RISCV, 0),

                "effi" => {
                    res = TileDesc::new_with_attr(
                        res.tile_type(),
                        res.isa(),
                        0,
                        res.attr() | TileAttr::EFFI,
                    )
                },
                "perf" => {
                    res = TileDesc::new_with_attr(
                        res.tile_type(),
                        res.isa(),
                        0,
                        res.attr() | TileAttr::PERF,
                    )
                },
                "nic" => {
                    res = TileDesc::new_with_attr(
                        res.tile_type(),
                        res.isa(),
                        0,
                        res.attr() | TileAttr::NIC,
                    )
                },
                "serial" => {
                    res = TileDesc::new_with_attr(
                        res.tile_type(),
                        res.isa(),
                        0,
                        res.attr() | TileAttr::SERIAL,
                    )
                },
                "kecacc" => {
                    res = TileDesc::new_with_attr(
                        res.tile_type(),
                        res.isa(),
                        0,
                        res.attr() | TileAttr::KECACC | TileAttr::IMEM,
                    )
                },
                "rot" => {
                    res = TileDesc::new_with_attr(
                        res.tile_type(),
                        res.isa(),
                        0,
                        res.attr() | TileAttr::ROT | TileAttr::IMEM,
                    )
                },

                "indir" => {
                    res = TileDesc::new_with_attr(
                        TileType::Comp,
                        TileISA::AccelIndir,
                        0,
                        TileAttr::IMEM,
                    )
                },
                "copy" => {
                    res = TileDesc::new_with_attr(
                        TileType::Comp,
                        TileISA::AccelCopy,
                        0,
                        TileAttr::IMEM,
                    )
                },
                "rot13" => {
                    res = TileDesc::new_with_attr(
                        TileType::Comp,
                        TileISA::AccelRot13,
                        0,
                        TileAttr::IMEM,
                    )
                },
                "idedev" => {
                    res = TileDesc::new_with_attr(
                        TileType::Comp,
                        TileISA::IDEDev,
                        0,
                        TileAttr::empty(),
                    )
                },
                "nicdev" => {
                    res = TileDesc::new_with_attr(
                        TileType::Comp,
                        TileISA::NICDev,
                        0,
                        TileAttr::empty(),
                    )
                },
                "serdev" => {
                    res = TileDesc::new_with_attr(
                        TileType::Comp,
                        TileISA::SerialDev,
                        0,
                        TileAttr::empty(),
                    )
                },
                "cfidev" => {
                    res = TileDesc::new_with_attr(
                        TileType::Comp,
                        TileISA::CFIDev,
                        0,
                        TileAttr::empty(),
                    )
                },

                _ => {},
            }
        }
        res
    }

    /// Returns the starting address and size of the standard receive buffer space
    pub fn rbuf_std_space(self) -> (VirtAddr, usize) {
        (self.rbuf_base(), cfg::RBUF_STD_SIZE)
    }

    /// Returns the starting address and size of the receive buffer space
    pub fn rbuf_space(self) -> (VirtAddr, usize) {
        let size = if self.has_virtmem() {
            cfg::RBUF_SIZE
        }
        else {
            cfg::RBUF_SIZE_SPM
        };
        (self.rbuf_base() + cfg::RBUF_STD_SIZE, size)
    }

    /// Returns the highest address of the stack
    pub fn stack_top(self) -> VirtAddr {
        let (addr, size) = self.stack_space();
        addr + size
    }

    /// Returns the starting address and size of the stack
    pub fn stack_space(self) -> (VirtAddr, usize) {
        (self.rbuf_base() - cfg::STACK_SIZE, cfg::STACK_SIZE)
    }

    fn rbuf_base(self) -> VirtAddr {
        if self.has_virtmem() {
            cfg::RBUF_STD_ADDR
        }
        else {
            let rbufs = cfg::RBUF_SIZE_SPM + cfg::RBUF_STD_SIZE;
            VirtAddr::from(cfg::MEM_OFFSET + self.mem_size() - rbufs)
        }
    }
}

impl fmt::Debug for TileDesc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TileDesc[type={:?}, isa={:?}, memsz={}, attr={:?}]",
            self.tile_type(),
            self.isa(),
            self.mem_size(),
            self.attr(),
        )
    }
}
