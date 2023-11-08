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

use core::fmt;

use base::mem::GlobAddr;

use crate::cap::{CapFlags, SelSpace, Selector};
use crate::cell::Ref;
use crate::col::Vec;
use crate::com::ep::EP;
use crate::com::gate::Gate;
use crate::errors::Error;
use crate::kif::INVALID_SEL;
use crate::mem::{self, GlobOff, MaybeUninit, VirtAddr};
use crate::syscalls;
use crate::tcu;
use crate::tiles::Activity;

pub use crate::kif::Perm;

/// Represents a contiguous region of memory, accessible via TCU
///
/// A memory gate provides access to a contiguous region of physical memory and allows RDMA-like
/// memory accesses via TCU. The physical memory can be located in a memory tile (e.g., DRAM), but
/// also in a compute tile. In the latter case it either refers to local memory (scratchpad) in the
/// destination tile or to physical memory located in another tile, but accessed through the cache
/// of the destination tile.
///
/// The following illustrates the difference with an example:
///
/// ```text
///    +-------------+        +--------------------------+
///    | Tile1       |        | Tile2                    |
///    | +---------+ |        | +---------+  +---------+ |
///    | |         | |        | |         |  |  Cache  | |
///    | |   TCU   | |    +---+->   TCU   +->+         | |
///    | |         | |    |   | |         |  |  BBCB   | |
///    | +----+----+ |    |   | +----+----+  |         | |
/// +--+-++ A | B ++-+----+   | |    |    |  |         | |
/// |  | +----+----+ |        | +----+----+  +----+----+ |
/// |  |             |        |                   |      |
/// |  +-------------+        +-------------------+------+
/// |                                             |
/// |  +------------------------------------------+------+
/// |  | Tile3                                    |      |
/// |  | +----------------------------------------+----+ |
/// |  | |                                        v    | |
/// +--+-+-->AAAA             DRAM              BBBB   | |
///    | |                                             | |
///    | +---------------------------------------------+ |
///    +-------------------------------------------------+
/// ```
///
/// The example above has two memory endpoints in Tile1's TCU (A and B). Endpoint A is directly
/// referring to the DRAM tile and thus sees the data `AAAA` in DRAM. In contrast, endpoint B refers
/// to the physical memory visible through the cache in Tile2. Therefore, endpoint B will see the
/// current state of the data in Tile2's cache (`BBCB`) instead of the potentially outdated data in
/// DRAM (`BBBB`).
///
/// Accessing physical memory through the cache of another tile therefore primarily exists because
/// tiles are not cache coherent. For example, it allows one application to get direct access to
/// data managed by another application.
///
/// The creation of `MemGate` therefore comes in two primary flavors: [`MemGate::new`] that
/// allocates new physical memory in DRAM and [`MemGate::new_foreign`] that provides access to a
/// physically contiguous physical memory region within another virtual address space.
///
/// Independent of the creation, every `MemGate` allows to issue DMA requests to the associated
/// memory region via [`MemGate::read`] and [`MemGate::write`].

pub struct MemGate {
    gate: Gate,
    resmng: bool,
}

/// The arguments for [`MemGate`] creations.
pub struct MGateArgs {
    size: usize,
    perm: Perm,
    sel: Selector,
}

impl MGateArgs {
    /// Creates a new `MGateArgs` object with default settings
    pub fn new(size: usize, perm: Perm) -> MGateArgs {
        MGateArgs {
            size,
            perm,
            sel: INVALID_SEL,
        }
    }

    /// Sets the capability selector that should be used for this [`MemGate`]. Otherwise and by
    /// default, [`SelSpace::get().alloc_sel`](crate::cap::SelSpace::alloc_sel) will be used to
    /// choose a free selector.
    pub fn sel(mut self, sel: Selector) -> Self {
        self.sel = sel;
        self
    }
}

impl MemGate {
    /// Creates a new `MemGate` that has access to a region of `size` bytes with permissions `perm`.
    ///
    /// This method will allocate `size` bytes with given permissions from the resource manager.
    pub fn new(size: usize, perm: Perm) -> Result<Self, Error> {
        Self::new_with(MGateArgs::new(size, perm))
    }

    /// Creates a new `MemGate` with given arguments.
    ///
    /// This method will allocate `size` bytes with given permissions from the resource manager.
    pub fn new_with(args: MGateArgs) -> Result<Self, Error> {
        let sel = if args.sel == INVALID_SEL {
            SelSpace::get().alloc_sel()
        }
        else {
            args.sel
        };

        Activity::own()
            .resmng()
            .unwrap()
            .alloc_mem(sel, args.size as GlobOff, args.perm)?;
        Ok(MemGate {
            gate: Gate::new(sel, CapFlags::empty()),
            resmng: true,
        })
    }

    /// Creates a new `MemGate` for the region `virt`..`virt`+`size` in the virtual address space of
    /// the given activity.
    ///
    /// The given region in virtual memory must be physically contiguous and page aligned. Note that
    /// the preferred interface for this functionality is [`Activity::get_mem`].
    pub fn new_foreign(
        act: Selector,
        virt: VirtAddr,
        size: GlobOff,
        perm: Perm,
    ) -> Result<Self, Error> {
        let sel = SelSpace::get().alloc_sel();
        syscalls::create_mgate(sel, act, virt, size, perm)?;
        Ok(MemGate::new_owned_bind(sel))
    }

    /// Binds a new `MemGate` to the given selector.
    pub fn new_bind(sel: Selector) -> Self {
        MemGate {
            gate: Gate::new(sel, CapFlags::KEEP_CAP),
            resmng: false,
        }
    }

    /// Binds a new `MemGate` to the given selector and revokes the cap on drop.
    pub fn new_owned_bind(sel: Selector) -> Self {
        MemGate {
            gate: Gate::new(sel, CapFlags::empty()),
            resmng: false,
        }
    }

    /// Binds a new `MemGate` to the boot module with given name.
    pub fn new_bind_bootmod(name: &str) -> Result<Self, Error> {
        let sel = SelSpace::get().alloc_sel();
        Activity::own().resmng().unwrap().use_mod(sel, name)?;
        Ok(MemGate {
            gate: Gate::new(sel, CapFlags::empty()),
            resmng: false,
        })
    }

    /// Returns the selector of this gate
    pub fn sel(&self) -> Selector {
        self.gate.sel()
    }

    /// Returns the endpoint of the gate. If the gate is not activated, `None` is returned.
    pub fn ep(&self) -> Option<Ref<'_, EP>> {
        self.gate.ep()
    }

    /// Sets or unsets the endpoint.
    pub(crate) fn set_ep(&mut self, ep: Option<EP>) {
        self.gate.set_ep(ep);
    }

    /// Returns the memory region (global address and size) this MemGate references.
    pub fn region(&self) -> Result<(GlobAddr, GlobOff), Error> {
        syscalls::mgate_region(self.sel())
    }

    /// Derives a new `MemGate` from `self` that has access to a subset of `self`'s the memory
    /// region and has a subset of `self`'s permissions. The subset of the memory region is defined
    /// by `offset` and `size` and the permissions by `perm`.
    ///
    /// Note that kernel makes sure that only owned permissions can be passed on to the derived
    /// `MemGate`.
    pub fn derive(&self, offset: GlobOff, size: usize, perm: Perm) -> Result<Self, Error> {
        let sel = SelSpace::get().alloc_sel();
        self.derive_for(Activity::own().sel(), sel, offset, size, perm)
    }

    /// Like [`MemGate::derive`], but assigns the new `MemGate` to the given activity and uses given
    /// selector.
    pub fn derive_for(
        &self,
        act: Selector,
        sel: Selector,
        offset: GlobOff,
        size: usize,
        perm: Perm,
    ) -> Result<Self, Error> {
        syscalls::derive_mem(act, sel, self.sel(), offset, size as GlobOff, perm)?;
        Ok(MemGate {
            gate: Gate::new(sel, CapFlags::empty()),
            resmng: false,
        })
    }

    /// Uses the TCU read command to read from the memory region at offset `off` and stores the read
    /// data into a vector. The number of bytes to read is defined by the number of items and the
    /// size of `T`.
    #[allow(clippy::uninit_vec)]
    pub fn read_into_vec<T>(&self, items: usize, off: GlobOff) -> Result<Vec<T>, Error> {
        let mut vec = Vec::<T>::with_capacity(items);
        // we deliberately use uninitialize memory here, because it's performance critical
        // safety: this is okay, because the TCU does not read from `vec`
        unsafe {
            vec.set_len(items)
        };
        self.read(&mut vec, off)?;
        Ok(vec)
    }

    /// Uses the TCU read command to read from the memory region at offset `off` and stores the read
    /// data into the slice `data`. The number of bytes to read is defined by `data`.
    pub fn read<T>(&self, data: &mut [T], off: GlobOff) -> Result<(), Error> {
        self.read_bytes(
            data.as_mut_ptr() as *mut u8,
            data.len() * mem::size_of::<T>(),
            off,
        )
    }

    /// Reads `mem::size_of::<T>()` bytes via the TCU read command from the memory region at offset
    /// `off` and returns the data as an object of `T`.
    pub fn read_obj<T>(&self, off: GlobOff) -> Result<T, Error> {
        #[allow(clippy::uninit_assumed_init)]
        // safety: will be initialized in read_bytes
        let mut obj: T = unsafe { MaybeUninit::uninit().assume_init() };
        self.read_bytes(&mut obj as *mut T as *mut u8, mem::size_of::<T>(), off)?;
        Ok(obj)
    }

    /// Reads `size` bytes via the TCU read command from the memory region at offset `off` and
    /// stores the read data into `data`.
    pub fn read_bytes(&self, data: *mut u8, size: usize, off: GlobOff) -> Result<(), Error> {
        let ep = self.activate()?;
        tcu::TCU::read(ep, data, size, off)
    }

    /// Writes `data` with the TCU write command to the memory region at offset `off`.
    pub fn write<T>(&self, data: &[T], off: GlobOff) -> Result<(), Error> {
        self.write_bytes(
            data.as_ptr() as *const u8,
            data.len() * mem::size_of::<T>(),
            off,
        )
    }

    /// Writes `obj` via the TCU write command to the memory region at offset `off`.
    pub fn write_obj<T>(&self, obj: &T, off: GlobOff) -> Result<(), Error> {
        self.write_bytes(obj as *const T as *const u8, mem::size_of::<T>(), off)
    }

    /// Writes the `size` bytes at `data` via the TCU write command to the memory region at offset
    /// `off`.
    pub fn write_bytes(&self, data: *const u8, size: usize, off: GlobOff) -> Result<(), Error> {
        let ep = self.activate()?;
        tcu::TCU::write(ep, data, size, off)
    }

    /// Activates the gate. Returns the chosen endpoint number.
    /// The endpoint can be delegated to other services (e.g. M3FS) to let them
    /// remotely configure it to point to memory in another tile.
    #[inline(always)]
    pub fn activate(&self) -> Result<tcu::EpId, Error> {
        self.gate.activate()
    }

    /// Deactivates this `MemGate` in case it was already activated
    pub fn deactivate(&mut self) {
        self.gate.release(true);
    }
}

impl Drop for MemGate {
    fn drop(&mut self) {
        if !self.gate.flags().contains(CapFlags::KEEP_CAP) && self.resmng {
            Activity::own().resmng().unwrap().free_mem(self.sel()).ok();
            self.gate.set_flags(CapFlags::KEEP_CAP);
        }
    }
}

impl fmt::Debug for MemGate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "MemGate[sel: {}, ep: {:?}]",
            self.sel(),
            self.gate.epid()
        )
    }
}
