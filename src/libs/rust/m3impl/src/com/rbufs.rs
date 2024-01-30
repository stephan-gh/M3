/*
 * Copyright (C) 2020-2022 Nils Asmussen, Barkhausen Institut
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

use crate::cap::Selector;
use crate::cell::LazyStaticRefCell;
use crate::cfg;
use crate::com::MemGate;
use crate::errors::Error;
use crate::kif::Perm;
use crate::mem::{GlobOff, MemMap, VirtAddr};
use crate::tiles::Activity;
use crate::util::math;

static BUFS: LazyStaticRefCell<MemMap<usize>> = LazyStaticRefCell::default();

/// A buffer to receive messages from a [`RecvGate`](crate::com::RecvGate).
///
/// For SPM tiles, the receive buffer will always be in the local SPM and thus there is no
/// [`MemGate`] used. For cache tiles, we allocate physical memory and map it into our address
/// space.
pub struct RecvBuf {
    addr: VirtAddr,
    size: usize,
    mgate: Option<MemGate>,
}

impl RecvBuf {
    /// Returns the base address of the receive buffer
    pub fn addr(&self) -> VirtAddr {
        self.addr
    }

    /// Returns the size of the receive buffer
    pub fn size(&self) -> usize {
        self.size
    }

    /// Returns the offset to specify on [`RecvGate`](crate::com::RecvGate) activation
    pub fn off(&self) -> GlobOff {
        match self.mgate {
            Some(_) => 0,
            None => self.addr.as_goff(),
        }
    }

    /// Returns the selector to specify on [`RecvGate`](crate::com::RecvGate) activation
    pub fn mem(&self) -> Option<Selector> {
        self.mgate.as_ref().map(|mg| mg.sel())
    }
}

impl fmt::Debug for RecvBuf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "RecvBuf[addr={}, size={}, sel={:?}]",
            self.addr,
            self.size,
            self.mem()
        )
    }
}

/// Allocates a new receive buffer with given size
pub(crate) fn alloc_rbuf(size: usize) -> Result<RecvBuf, Error> {
    let vm = Activity::own().tile_desc().has_virtmem();
    let align = if vm { cfg::PAGE_SIZE } else { 1 };
    let addr = VirtAddr::from(BUFS.borrow_mut().allocate(size, align)?);

    let mgate = if vm {
        match map_rbuf(addr, size) {
            Ok(mgate) => Some(mgate),
            Err(e) => {
                BUFS.borrow_mut().free(addr.as_local(), size);
                return Err(e);
            },
        }
    }
    else {
        None
    };

    Ok(RecvBuf { addr, size, mgate })
}

fn map_rbuf(addr: VirtAddr, size: usize) -> Result<MemGate, Error> {
    let size = math::round_up(size, cfg::PAGE_SIZE);
    let mgate = MemGate::new(size as GlobOff, Perm::R)?;
    crate::syscalls::create_map(
        addr,
        Activity::own().sel(),
        mgate.sel(),
        0,
        (size / cfg::PAGE_SIZE) as Selector,
        Perm::R,
    )?;
    #[cfg(feature = "linux")]
    base::linux::mmap::mmap_tcu(
        base::linux::tcu_fd(),
        addr,
        size,
        base::linux::mmap::MemType::Custom,
        Perm::R,
    )?;
    Ok(mgate)
}

/// Frees the given receive buffer
pub(crate) fn free_rbuf(rbuf: &RecvBuf) {
    #[cfg(feature = "linux")]
    base::linux::mmap::munmap(rbuf.addr(), rbuf.size());
    BUFS.borrow_mut().free(rbuf.addr.as_local(), rbuf.size);
}

pub(crate) fn init() {
    let (addr, size) = Activity::own().tile_desc().rbuf_space();
    BUFS.set(MemMap::new(addr.as_local(), size));
}
