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
use com::gate::Gate;
use core::fmt;
use core::mem::MaybeUninit;
use dtu;
use errors::{Code, Error};
use goff;
use kif::{self, INVALID_SEL};
use syscalls;
use util;
use vpe;

pub use kif::Perm;

/// A memory gate (`MemGate`) has access to a contiguous memory region and allows RDMA-like memory
/// accesses via DTU.
pub struct MemGate {
    gate: Gate,
    revoke: bool,
}

/// The arguments for `MemGate` creations.
pub struct MGateArgs {
    size: usize,
    addr: goff,
    perm: Perm,
    sel: Selector,
    flags: CapFlags,
}

impl MGateArgs {
    /// Creates a new `MGateArgs` object with default settings
    pub fn new(size: usize, perm: Perm) -> MGateArgs {
        MGateArgs {
            size,
            addr: !0,
            perm,
            sel: INVALID_SEL,
            flags: CapFlags::empty(),
        }
    }

    /// Sets the address to `addr` to request a specific memory region. Otherwise and by default,
    /// any free memory region of the requested size will be used.
    pub fn addr(mut self, addr: goff) -> Self {
        self.addr = addr;
        self
    }

    /// Sets the capability selector that should be used for this `MemGate`. Otherwise and by default,
    /// [`vpe::VPE::alloc_sel`] will be used to choose a free selector.
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
            vpe::VPE::cur().alloc_sel()
        }
        else {
            args.sel
        };

        vpe::VPE::cur().resmng().alloc_mem(sel, args.addr, args.size, args.perm)?;
        Ok(MemGate {
            gate: Gate::new(sel, args.flags),
            revoke: false,
        })
    }

    /// Binds a new `MemGate` to the given selector.
    pub fn new_bind(sel: Selector) -> Self {
        MemGate {
            gate: Gate::new(sel, CapFlags::KEEP_CAP),
            revoke: true,
        }
    }

    /// Returns the selector of this gate
    pub fn sel(&self) -> Selector {
        self.gate.sel()
    }
    /// Returns the endpoint of the gate. If the gate is not activated, `None` is returned.
    pub fn ep(&self) -> Option<dtu::EpId> {
        self.gate.ep()
    }

    pub(crate) fn set_ep(&mut self, ep: dtu::EpId) {
        self.gate.set_ep(ep);
    }
    pub(crate) fn unset_ep(&mut self) {
        self.gate.unset_ep();
    }

    /// Derives a new `MemGate` from `self` that has access to a subset of `self`'s the memory
    /// region and has a subset of `self`'s permissions. The subset of the memory region is defined
    /// by `offset` and `size` and the permissions by `perm`.
    ///
    /// Note that kernel makes sure that only owned permissions can be passed on to the derived
    /// `MemGate`.
    pub fn derive(&self, offset: goff, size: usize, perm: Perm) -> Result<Self, Error> {
        let sel = vpe::VPE::cur().alloc_sel();
        self.derive_for(vpe::VPE::cur().sel(), sel, offset, size, perm)
    }

    /// Like [`MemGate::derive`], but assigns the new `MemGate` to the given VPE and uses given
    /// selector.
    pub fn derive_for(&self, vpe: Selector, sel: Selector, offset: goff,
                      size: usize, perm: Perm) -> Result<Self, Error> {
        syscalls::derive_mem(vpe, sel, self.sel(), offset, size, perm)?;
        Ok(MemGate {
            gate: Gate::new(sel, CapFlags::empty()),
            revoke: true,
        })
    }

    /// Rebinds this gate to capability selector `sel`
    pub fn rebind(&mut self, sel: Selector) -> Result<(), Error> {
        self.gate.rebind(sel)
    }

    /// Uses the DTU read command to read from the memory region at offset `off` and stores the read
    /// data into the slice `data`. The number of bytes to read is defined by `data`.
    pub fn read<T>(&self, data: &mut [T], off: goff) -> Result<(), Error> {
        self.read_bytes(data.as_mut_ptr() as *mut u8, data.len() * util::size_of::<T>(), off)
    }

    /// Reads `util::size_of::<T>()` bytes via the DTU read command from the memory region at offset
    /// `off` and returns the data as an object of `T`.
    pub fn read_obj<T>(&self, off: goff) -> Result<T, Error> {
        let mut obj: T = unsafe { MaybeUninit::uninit().assume_init() };
        self.read_bytes(&mut obj as *mut T as *mut u8, util::size_of::<T>(), off)?;
        Ok(obj)
    }

    /// Reads `size` bytes via the DTU read command from the memory region at offset `off` and
    /// stores the read data into `data`.
    pub fn read_bytes(&self, mut data: *mut u8, mut size: usize, mut off: goff) -> Result<(), Error> {
        let ep = self.gate.activate()?;

        loop {
            match dtu::DTU::read(ep, data, size, off, dtu::CmdFlags::empty()) {
                Ok(_)                                   => return Ok(()),
                Err(ref e) if e.code() == Code::VPEGone => {
                    // simply retry the write if the forward failed (pagefault)
                    if self.forward_read(&mut data, &mut size, &mut off).is_ok() && size == 0 {
                        break Ok(())
                    }
                },
                Err(e)                                  => return Err(e),
            }
        }
    }

    /// Writes `data` with the DTU write command to the memory region at offset `off`.
    pub fn write<T>(&self, data: &[T], off: goff) -> Result<(), Error> {
        self.write_bytes(data.as_ptr() as *const u8, data.len() * util::size_of::<T>(), off)
    }

    /// Writes `obj` via the DTU write command to the memory region at offset `off`.
    pub fn write_obj<T>(&self, obj: &T, off: goff) -> Result<(), Error> {
        self.write_bytes(obj as *const T as *const u8, util::size_of::<T>(), off)
    }

    /// Writes the `size` bytes at `data` via the DTU write command to the memory region at offset
    /// `off`.
    pub fn write_bytes(&self, mut data: *const u8, mut size: usize, mut off: goff) -> Result<(), Error> {
        let ep = self.gate.activate()?;

        loop {
            match dtu::DTU::write(ep, data, size, off, dtu::CmdFlags::empty()) {
                Ok(_)                                   => return Ok(()),
                Err(ref e) if e.code() == Code::VPEGone => {
                    // simply retry the write if the forward failed (pagefault)
                    if self.forward_write(&mut data, &mut size, &mut off).is_ok() && size == 0 {
                        break Ok(());
                    }
                },
                Err(e)                                  => return Err(e),
            }
        }
    }

    fn forward_read(&self, data: &mut *mut u8, size: &mut usize, off: &mut goff) -> Result<(), Error> {
        let amount = util::min(kif::syscalls::MAX_MSG_SIZE, *size);
        syscalls::forward_read(
            self.sel(), unsafe { util::slice_for_mut(*data, amount) }, *off,
            kif::syscalls::ForwardMemFlags::empty(), 0
        )?;
        *data = unsafe { (*data).add(amount) };
        *off += amount as goff;
        *size -= amount;
        Ok(())
    }

    fn forward_write(&self, data: &mut *const u8, size: &mut usize, off: &mut goff) -> Result<(), Error> {
        let amount = util::min(kif::syscalls::MAX_MSG_SIZE, *size);
        syscalls::forward_write(
            self.sel(), unsafe { util::slice_for(*data, amount) }, *off,
            kif::syscalls::ForwardMemFlags::empty(), 0
        )?;
        *data = unsafe { (*data).add(amount) };
        *off += amount as goff;
        *size -= amount;
        Ok(())
    }
}

impl Drop for MemGate {
    fn drop(&mut self) {
        if !self.gate.flags().contains(CapFlags::KEEP_CAP) && !self.revoke {
            vpe::VPE::cur().resmng().free_mem(self.sel()).ok();
            self.gate.set_flags(CapFlags::KEEP_CAP);
        }
    }
}

impl fmt::Debug for MemGate {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "MemGate[sel: {}, ep: {:?}]", self.sel(), self.gate.ep())
    }
}
