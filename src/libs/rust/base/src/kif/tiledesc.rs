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

use crate::cfg;
use crate::serialize::{Deserialize, Serialize};

int_enum! {
    /// The different types of tiles
    pub struct TileType : TileDescRaw {
        /// Compute tile
        const COMP          = 0x0;
        /// Memory tile
        const MEM           = 0x1;
    }
}

int_enum! {
    /// The supported instruction set architectures (ISAs)
    pub struct TileISA : TileDescRaw {
        /// Dummy ISA to represent memory tiles
        const NONE          = 0x0;
        /// RISCV as supported on hw and gem5
        const RISCV         = 0x1;
        /// x86_64 as supported by gem5
        const X86           = 0x2;
        /// ARMv7 as supported by gem5
        const ARM           = 0x3;
        /// Dummy ISA to represent the indirect-chaining fixed-function accelerator
        const ACCEL_INDIR   = 0x4;
        /// Dummy ISA to represent the COPY fixed-function accelerator
        const ACCEL_COPY    = 0x5;
        /// Dummy ISA to represent the ROT-13 fixed-function accelerator
        const ACCEL_ROT13   = 0x6;
        /// Dummy ISA to represent the IDE controller
        const IDE_DEV       = 0x7;
        /// Dummy ISA to represent the NIC
        const NIC_DEV       = 0x8;
        /// Dummy ISA to represent the serial input device
        const SERIAL_DEV    = 0x9;
    }
}

bitflags! {
    pub struct TileAttr : TileDescRaw {
        const BOOM          = 1 << 0;
        const ROCKET        = 1 << 1;
        const NIC           = 1 << 2;
        const SERIAL        = 1 << 3;
        const IMEM          = 1 << 4;
        /// Tile contains a Keccak Accelerator (KecAcc)
        const KECACC        = 1 << 5;
    }
}

/// The underlying type of [`TileDesc`]
///
/// +---------------------------+------------+-----+------+
/// | memory size (in 4K pages) | attributes | ISA | type |
/// +---------------------------+------------+-----+------+
/// 64                         28           20     6      0
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
pub struct TileDesc {
    val: TileDescRaw,
}

impl TileDesc {
    /// Creates a new tile description from the given type, ISA, and memory size.
    pub const fn new(ty: TileType, isa: TileISA, memsize: usize) -> TileDesc {
        let mem_pages = memsize >> 12;
        let val = ty.val | (isa.val << 6) | (mem_pages as TileDescRaw) << 28;
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
        let val = ty.val | (isa.val << 6) | (attr.bits() << 20) | (mem_pages as TileDescRaw) << 28;
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
        TileISA::from((self.val >> 6) & 0x3FFF)
    }

    pub fn attr(self) -> TileAttr {
        TileAttr::from_bits_truncate((self.val >> 20) & 0xFF)
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
        self.isa() == TileISA::NIC_DEV
            || self.isa() == TileISA::IDE_DEV
            || self.isa() == TileISA::SERIAL_DEV
    }

    /// Return if the tile supports activities
    pub fn supports_activities(self) -> bool {
        self.tile_type() != TileType::MEM
    }

    /// Return if the tile supports the context switching protocol
    pub fn supports_tilemux(self) -> bool {
        self.supports_activities() && !self.is_device()
    }

    /// Returns whether the tile has an internal memory (SPM, DRAM, ...)
    pub fn has_memory(self) -> bool {
        self.tile_type() == TileType::MEM || self.attr().contains(TileAttr::IMEM)
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
                "arm" => res = TileDesc::new(TileType::COMP, TileISA::ARM, 0),
                "x86" => res = TileDesc::new(TileType::COMP, TileISA::X86, 0),
                "riscv" => res = TileDesc::new(TileType::COMP, TileISA::RISCV, 0),

                "rocket" => {
                    res = TileDesc::new_with_attr(
                        res.tile_type(),
                        res.isa(),
                        0,
                        res.attr() | TileAttr::ROCKET,
                    )
                },
                "boom" => {
                    res = TileDesc::new_with_attr(
                        res.tile_type(),
                        res.isa(),
                        0,
                        res.attr() | TileAttr::BOOM,
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

                "indir" => {
                    res = TileDesc::new_with_attr(
                        TileType::COMP,
                        TileISA::ACCEL_INDIR,
                        0,
                        TileAttr::IMEM,
                    )
                },
                "copy" => {
                    res = TileDesc::new_with_attr(
                        TileType::COMP,
                        TileISA::ACCEL_COPY,
                        0,
                        TileAttr::IMEM,
                    )
                },
                "rot13" => {
                    res = TileDesc::new_with_attr(
                        TileType::COMP,
                        TileISA::ACCEL_ROT13,
                        0,
                        TileAttr::IMEM,
                    )
                },
                "idedev" => {
                    res =
                        TileDesc::new_with_attr(TileType::COMP, TileISA::IDE_DEV, 0, TileAttr::IMEM)
                },
                "nicdev" => {
                    res =
                        TileDesc::new_with_attr(TileType::COMP, TileISA::NIC_DEV, 0, TileAttr::IMEM)
                },
                "serdev" => {
                    res = TileDesc::new_with_attr(
                        TileType::COMP,
                        TileISA::SERIAL_DEV,
                        0,
                        TileAttr::IMEM,
                    )
                },

                _ => {},
            }
        }
        res
    }

    /// Returns the starting address and size of the standard receive buffer space
    pub fn rbuf_std_space(self) -> (usize, usize) {
        (self.rbuf_base(), cfg::RBUF_STD_SIZE)
    }

    /// Returns the starting address and size of the receive buffer space
    pub fn rbuf_space(self) -> (usize, usize) {
        let size = if self.has_virtmem() {
            cfg::RBUF_SIZE
        }
        else {
            cfg::RBUF_SIZE_SPM
        };
        (self.rbuf_base() + cfg::RBUF_STD_SIZE, size)
    }

    /// Returns the highest address of the stack
    pub fn stack_top(self) -> usize {
        let (addr, size) = self.stack_space();
        addr + size
    }

    /// Returns the starting address and size of the stack
    pub fn stack_space(self) -> (usize, usize) {
        (self.rbuf_base() - cfg::STACK_SIZE, cfg::STACK_SIZE)
    }

    fn rbuf_base(self) -> usize {
        if self.has_virtmem() {
            cfg::RBUF_STD_ADDR
        }
        else {
            let rbufs = cfg::TILEMUX_RBUF_SIZE + cfg::RBUF_SIZE_SPM + cfg::RBUF_STD_SIZE;
            cfg::MEM_OFFSET + self.mem_size() - rbufs
        }
    }
}

impl fmt::Debug for TileDesc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TileDesc[type={}, isa={}, memsz={}, attr={:?}]",
            self.tile_type(),
            self.isa(),
            self.mem_size(),
            self.attr(),
        )
    }
}
