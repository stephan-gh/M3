/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

use cfg_if::cfg_if;
use core::fmt;

use crate::cfg;

int_enum! {
    /// The different types of PEs
    pub struct PEType : PEDescRaw {
        /// Compute PE with internal memory
        const COMP_IMEM     = 0x0;
        /// Compute PE with cache and external memory
        const COMP_EMEM     = 0x1;
        /// Memory PE
        const MEM           = 0x2;
    }
}

int_enum! {
    /// The supported instruction set architectures (ISAs)
    pub struct PEISA : PEDescRaw {
        /// Dummy ISA to represent memory PEs
        const NONE          = 0x0;
        /// x86_64 as supported by gem5
        const X86           = 0x1;
        /// ARMv7 as supported by gem5
        const ARM           = 0x2;
        /// RISCV as supported on gem5
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
    }
}

/// The underlying type of [`PEDesc`]
pub type PEDescRaw = u64;

/// Describes a processing element (PE).
///
/// This struct is used for the [`create_vpe`] syscall to let the kernel know about the desired PE
/// type. Additionally, it is used to tell a VPE about the attributes of the PE it has been assigned
/// to.
///
/// [`create_vpe`]: ../../m3/syscalls/fn.create_vpe.html
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PEDesc {
    val: PEDescRaw,
}

impl PEDesc {
    /// Creates a new PE description from the given type, ISA, and memory size.
    pub const fn new(ty: PEType, isa: PEISA, memsize: usize) -> PEDesc {
        let val = ty.val | (isa.val << 3) | memsize as PEDescRaw;
        Self::new_from(val)
    }

    /// Creates a new PE description from the given raw value
    pub const fn new_from(val: PEDescRaw) -> PEDesc {
        PEDesc { val }
    }

    /// Returns the raw value
    pub fn value(self) -> PEDescRaw {
        self.val
    }

    pub fn pe_type(self) -> PEType {
        PEType::from(self.val & 0x7)
    }

    pub fn isa(self) -> PEISA {
        PEISA::from((self.val >> 3) & 0xF)
    }

    /// Returns the size of the internal memory (0 if none is present)
    pub fn mem_size(self) -> usize {
        (self.val & !0xFFF) as usize
    }

    /// Returns whether the PE executes software
    pub fn is_programmable(self) -> bool {
        matches!(self.isa(), PEISA::X86 | PEISA::ARM | PEISA::RISCV)
    }

    /// Return if the PE supports multiple contexts
    pub fn is_device(self) -> bool {
        self.isa() == PEISA::NIC_DEV || self.isa() == PEISA::IDE_DEV
    }

    /// Return if the PE supports VPEs
    pub fn supports_vpes(self) -> bool {
        self.pe_type() != PEType::MEM
    }

    /// Return if the PE supports the context switching protocol
    pub fn supports_pemux(self) -> bool {
        self.supports_vpes() && !self.is_device()
    }

    /// Returns whether the PE has an internal memory (SPM, DRAM, ...)
    pub fn has_mem(self) -> bool {
        self.pe_type() == PEType::COMP_IMEM || self.pe_type() == PEType::MEM
    }

    /// Returns whether the PE has a cache
    pub fn has_cache(self) -> bool {
        self.pe_type() == PEType::COMP_EMEM
    }

    /// Returns whether the PE supports virtual memory (either by TCU or MMU)
    pub fn has_virtmem(self) -> bool {
        self.has_cache()
    }

    /// Returns the starting address and size of the standard receive buffer space
    pub fn rbuf_std_space(self) -> (usize, usize) {
        cfg_if! {
            if #[cfg(target_os = "linux")] {
                (cfg::RBUF_STD_ADDR, cfg::RBUF_STD_SIZE)
            }
            else {
                if self.has_virtmem() {
                    (cfg::RBUF_STD_ADDR, cfg::RBUF_STD_SIZE)
                }
                else {
                    let rbufs = cfg::PEMUX_RBUF_SIZE + cfg::RBUF_SIZE_SPM + cfg::RBUF_STD_SIZE;
                    (cfg::MEM_OFFSET + self.mem_size() - rbufs, cfg::RBUF_STD_SIZE)
                }
            }
        }
    }

    /// Returns the starting address and size of the receive buffer space
    pub fn rbuf_space(self) -> (usize, usize) {
        cfg_if! {
            if #[cfg(target_os = "linux")] {
                (cfg::RBUF_ADDR, cfg::RBUF_SIZE)
            }
            else {
                if self.has_virtmem() {
                    (cfg::RBUF_ADDR, cfg::RBUF_SIZE)
                }
                else {
                    let rbufs = cfg::PEMUX_RBUF_SIZE + cfg::RBUF_SIZE_SPM;
                    (cfg::MEM_OFFSET + self.mem_size() - rbufs, cfg::RBUF_SIZE_SPM)
                }
            }
        }
    }
}

impl fmt::Debug for PEDesc {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "PEDesc[type={}, isa={}, memsz={}]",
            self.pe_type(),
            self.isa(),
            self.mem_size()
        )
    }
}
