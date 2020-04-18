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

use cap::{CapFlags, Selector};
use com::ep::EP;
use com::gate::Gate;
use core::fmt;
use core::mem::MaybeUninit;
use errors::Error;
use goff;
use kif::INVALID_SEL;
use pes::VPE;
use syscalls;
use tcu;
use util;

pub use kif::Perm;

bitflags! {
    pub struct MGateFlags : u64 {
        /// Pagefaults result in an abort
        const NOPF = tcu::CmdFlags::NOPF.bits();
        /// revoke the `MemGate` on destruction
        const REVOKE = 0x80;
    }
}

/// A memory gate (`MemGate`) has access to a contiguous memory region and allows RDMA-like memory
/// accesses via TCU.
pub struct MemGate {
    gate: Gate,
    flags: MGateFlags,
}

/// The arguments for `MemGate` creations.
pub struct MGateArgs {
    size: usize,
    addr: goff,
    perm: Perm,
    sel: Selector,
}

impl MGateArgs {
    /// Creates a new `MGateArgs` object with default settings
    pub fn new(size: usize, perm: Perm) -> MGateArgs {
        MGateArgs {
            size,
            addr: !0,
            perm,
            sel: INVALID_SEL,
        }
    }

    /// Sets the address to `addr` to request a specific memory region. Otherwise and by default,
    /// any free memory region of the requested size will be used.
    pub fn addr(mut self, addr: goff) -> Self {
        self.addr = addr;
        self
    }

    /// Sets the capability selector that should be used for this `MemGate`. Otherwise and by
    /// default, [`VPE::alloc_sel`] will be used to choose a free selector.
    pub fn sel(mut self, sel: Selector) -> Self {
        self.sel = sel;
        self
    }
}

impl MemGate {
    /// Creates a new `MemGate` that has access to a region of `size` bytes with permissions `perm`.
    pub fn new(size: usize, perm: Perm) -> Result<Self, Error> {
        Self::new_with(MGateArgs::new(size, perm))
    }

    /// Creates a new `MemGate` with given arguments.
    pub fn new_with(args: MGateArgs) -> Result<Self, Error> {
        let sel = if args.sel == INVALID_SEL {
            VPE::cur().alloc_sel()
        }
        else {
            args.sel
        };

        VPE::cur()
            .resmng()
            .alloc_mem(sel, args.addr, args.size, args.perm)?;
        Ok(MemGate {
            gate: Gate::new(sel, CapFlags::empty()),
            flags: MGateFlags::empty(),
        })
    }

    /// Binds a new `MemGate` to the given selector.
    pub fn new_bind(sel: Selector) -> Self {
        MemGate {
            gate: Gate::new(sel, CapFlags::KEEP_CAP),
            flags: MGateFlags::REVOKE,
        }
    }

    /// Binds a new `MemGate` to the given selector and revokes the cap on drop.
    pub fn new_owned_bind(sel: Selector) -> Self {
        MemGate {
            gate: Gate::new(sel, CapFlags::empty()),
            flags: MGateFlags::REVOKE,
        }
    }

    /// Returns the selector of this gate
    pub fn sel(&self) -> Selector {
        self.gate.sel()
    }

    /// Returns the endpoint of the gate. If the gate is not activated, `None` is returned.
    pub(crate) fn ep(&self) -> Option<&EP> {
        self.gate.ep()
    }

    /// Sets the flags to use for memory requests.
    pub fn set_flags(&mut self, flags: MGateFlags) {
        self.flags = flags | (self.flags & MGateFlags::REVOKE);
    }

    /// Derives a new `MemGate` from `self` that has access to a subset of `self`'s the memory
    /// region and has a subset of `self`'s permissions. The subset of the memory region is defined
    /// by `offset` and `size` and the permissions by `perm`.
    ///
    /// Note that kernel makes sure that only owned permissions can be passed on to the derived
    /// `MemGate`.
    pub fn derive(&self, offset: goff, size: usize, perm: Perm) -> Result<Self, Error> {
        let sel = VPE::cur().alloc_sel();
        self.derive_for(VPE::cur().sel(), sel, offset, size, perm)
    }

    /// Like [`MemGate::derive`], but assigns the new `MemGate` to the given VPE and uses given
    /// selector.
    pub fn derive_for(
        &self,
        vpe: Selector,
        sel: Selector,
        offset: goff,
        size: usize,
        perm: Perm,
    ) -> Result<Self, Error> {
        syscalls::derive_mem(vpe, sel, self.sel(), offset, size, perm)?;
        Ok(MemGate {
            gate: Gate::new(sel, CapFlags::empty()),
            flags: MGateFlags::REVOKE,
        })
    }

    /// Uses the TCU read command to read from the memory region at offset `off` and stores the read
    /// data into the slice `data`. The number of bytes to read is defined by `data`.
    pub fn read<T>(&self, data: &mut [T], off: goff) -> Result<(), Error> {
        self.read_bytes(
            data.as_mut_ptr() as *mut u8,
            data.len() * util::size_of::<T>(),
            off,
        )
    }

    /// Reads `util::size_of::<T>()` bytes via the TCU read command from the memory region at offset
    /// `off` and returns the data as an object of `T`.
    pub fn read_obj<T>(&self, off: goff) -> Result<T, Error> {
        #[allow(clippy::uninit_assumed_init)]
        // safety: will be initialized in read_bytes
        let mut obj: T = unsafe { MaybeUninit::uninit().assume_init() };
        self.read_bytes(&mut obj as *mut T as *mut u8, util::size_of::<T>(), off)?;
        Ok(obj)
    }

    /// Reads `size` bytes via the TCU read command from the memory region at offset `off` and
    /// stores the read data into `data`.
    pub fn read_bytes(&self, data: *mut u8, size: usize, off: goff) -> Result<(), Error> {
        tcu::TCUIf::read(self, data, size, off, self.cmd_flags())
    }

    /// Writes `data` with the TCU write command to the memory region at offset `off`.
    pub fn write<T>(&self, data: &[T], off: goff) -> Result<(), Error> {
        self.write_bytes(
            data.as_ptr() as *const u8,
            data.len() * util::size_of::<T>(),
            off,
        )
    }

    /// Writes `obj` via the TCU write command to the memory region at offset `off`.
    pub fn write_obj<T>(&self, obj: &T, off: goff) -> Result<(), Error> {
        self.write_bytes(obj as *const T as *const u8, util::size_of::<T>(), off)
    }

    /// Writes the `size` bytes at `data` via the TCU write command to the memory region at offset
    /// `off`.
    pub fn write_bytes(&self, data: *const u8, size: usize, off: goff) -> Result<(), Error> {
        tcu::TCUIf::write(self, data, size, off, self.cmd_flags())
    }

    pub(crate) fn activate(&self) -> Result<&EP, Error> {
        self.gate.activate()
    }

    /// Deactivates this `MemGate` in case it was already activated
    pub fn deactivate(&mut self) {
        self.gate.release(false);
    }

    fn cmd_flags(&self) -> tcu::CmdFlags {
        tcu::CmdFlags::from_bits_truncate(self.flags.bits() & MGateFlags::NOPF.bits())
    }
}

impl Drop for MemGate {
    fn drop(&mut self) {
        if !self.gate.flags().contains(CapFlags::KEEP_CAP)
            && !self.flags.contains(MGateFlags::REVOKE)
        {
            VPE::cur().resmng().free_mem(self.sel()).ok();
            self.gate.set_flags(CapFlags::KEEP_CAP);
        }
    }
}

impl fmt::Debug for MemGate {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(
            f,
            "MemGate[sel: {}, ep: {:?}]",
            self.sel(),
            self.gate.ep_id()
        )
    }
}
