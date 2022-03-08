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
use cfg_if::cfg_if;
use core::fmt;

use crate::cfg;

int_enum! {
    /// The different types of tiles
    pub struct TileType : TileDescRaw {
        /// Compute tile with internal memory
        const COMP_IMEM     = 0x0;
        /// Compute tile with cache and external memory
        const COMP_EMEM     = 0x1;
        /// Memory tile
        const MEM           = 0x2;
    }
}

int_enum! {
    /// The supported instruction set architectures (ISAs)
    pub struct TileISA : TileDescRaw {
        /// Dummy ISA to represent memory tiles
        const NONE          = 0x0;
        /// x86_64 as supported by gem5
        const X86           = 0x1;
        /// ARMv7 as supported by gem5
        const ARM           = 0x2;
        /// RISCV as supported on hw and gem5
        const RISCV         = 0x3;
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
        const BOOM          = 0x1;
        const ROCKET        = 0x2;
        const NIC           = 0x4;
        /// Tile contains a Keccak Accelerator (KecAcc)
        const KECACC        = 0x8;
    }
}

/// The underlying type of [`TileDesc`]
pub type TileDescRaw = u64;

/// Describes a tile.
///
/// This struct is used for the [`create_activity`] syscall to let the kernel know about the desired tile
/// type. Additionally, it is used to tell a activity about the attributes of the tile it has been assigned
/// to.
///
/// [`create_activity`]: ../../m3/syscalls/fn.create_activity.html
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct TileDesc {
    val: TileDescRaw,
}

impl TileDesc {
    /// Creates a new tile description from the given type, ISA, and memory size.
    pub const fn new(ty: TileType, isa: TileISA, memsize: usize) -> TileDesc {
        let val = ty.val | (isa.val << 3) | memsize as TileDescRaw;
        Self::new_from(val)
    }

    /// Creates a new tile description from the given type, ISA, memory size, and attributes.
    pub const fn new_with_attr(
        ty: TileType,
        isa: TileISA,
        memsize: usize,
        attr: TileAttr,
    ) -> TileDesc {
        let val = ty.val | (isa.val << 3) | (attr.bits() << 7) | memsize as TileDescRaw;
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
        TileType::from(self.val & 0x7)
    }

    pub fn isa(self) -> TileISA {
        TileISA::from((self.val >> 3) & 0xF)
    }

    pub fn attr(self) -> TileAttr {
        TileAttr::from_bits_truncate((self.val >> 7) & 0xF)
    }

    /// Returns the size of the internal memory (0 if none is present)
    pub fn mem_size(self) -> usize {
        (self.val & !0xFFF) as usize
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
    pub fn has_mem(self) -> bool {
        self.tile_type() == TileType::COMP_IMEM || self.tile_type() == TileType::MEM
    }

    /// Returns whether the tile has a cache
    pub fn has_cache(self) -> bool {
        self.tile_type() == TileType::COMP_EMEM
    }

    /// Returns whether the tile supports virtual memory (either by TCU or MMU)
    pub fn has_virtmem(self) -> bool {
        self.has_cache()
    }

    /// Derives a new TileDesc from this by changing it based on the given properties.
    pub fn with_properties(&self, props: &str) -> TileDesc {
        let mut res = *self;
        for prop in props.split('+') {
            match prop {
                "imem" => res = TileDesc::new(TileType::COMP_IMEM, res.isa(), 0),
                "emem" | "vm" => res = TileDesc::new(TileType::COMP_EMEM, res.isa(), 0),

                "arm" => res = TileDesc::new(res.tile_type(), TileISA::ARM, 0),
                "x86" => res = TileDesc::new(res.tile_type(), TileISA::X86, 0),
                "riscv" => res = TileDesc::new(res.tile_type(), TileISA::RISCV, 0),

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
                "kecacc" => {
                    res = TileDesc::new_with_attr(
                        res.tile_type(),
                        res.isa(),
                        0,
                        res.attr() | TileAttr::KECACC,
                    )
                },

                "indir" => res = TileDesc::new(TileType::COMP_IMEM, TileISA::ACCEL_INDIR, 0),
                "copy" => res = TileDesc::new(TileType::COMP_IMEM, TileISA::ACCEL_COPY, 0),
                "rot13" => res = TileDesc::new(TileType::COMP_IMEM, TileISA::ACCEL_ROT13, 0),
                "idedev" => res = TileDesc::new(TileType::COMP_IMEM, TileISA::IDE_DEV, 0),
                "nicdev" => res = TileDesc::new(TileType::COMP_IMEM, TileISA::NIC_DEV, 0),

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
        cfg_if! {
            if #[cfg(target_vendor = "host")] {
                cfg::RBUF_STD_ADDR
            }
            else {
                if self.has_virtmem() {
                    cfg::RBUF_STD_ADDR
                }
                else {
                    let rbufs = cfg::TILEMUX_RBUF_SIZE + cfg::RBUF_SIZE_SPM + cfg::RBUF_STD_SIZE;
                    cfg::MEM_OFFSET + self.mem_size() - rbufs
                }
            }
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
