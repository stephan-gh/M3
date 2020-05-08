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

use cap::Selector;
use cell::LazyStaticCell;
use cfg;
use com::MemGate;
use core::fmt;
use errors::Error;
use kif::Perm;
use math;
use mem::MemMap;
use pes::VPE;
use syscalls;

static BUFS: LazyStaticCell<MemMap> = LazyStaticCell::default();

pub struct RecvBuf {
    addr: usize,
    size: usize,
    mgate: Option<MemGate>,
}

impl RecvBuf {
    pub fn addr(&self) -> usize {
        self.addr
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn off(&self) -> usize {
        match self.mgate {
            Some(_) => 0,
            None => self.addr,
        }
    }

    pub fn mem(&self) -> Option<Selector> {
        self.mgate.as_ref().map(|mg| mg.sel())
    }
}

impl fmt::Debug for RecvBuf {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(
            f,
            "RecvBuf[addr={}, size={}, sel={:?}]",
            self.addr,
            self.size,
            self.mem()
        )
    }
}

pub fn alloc_rbuf(size: usize) -> Result<RecvBuf, Error> {
    let vm = VPE::cur().pe_desc().has_virtmem();
    let align = if vm { cfg::PAGE_SIZE as u64 } else { 1 };
    let addr = BUFS.get_mut().allocate(size as u64, align)? as usize;

    let mgate = if vm {
        match map_rbuf(addr, size) {
            Ok(mgate) => Some(mgate),
            Err(e) => {
                BUFS.get_mut().free(addr as u64, size as u64);
                return Err(e);
            },
        }
    }
    else {
        None
    };

    Ok(RecvBuf { addr, size, mgate })
}

fn map_rbuf(addr: usize, size: usize) -> Result<MemGate, Error> {
    let size = math::round_up(size, cfg::PAGE_SIZE);
    let mgate = MemGate::new(size, Perm::R)?;
    syscalls::create_map(
        (addr / cfg::PAGE_SIZE) as Selector,
        VPE::cur().sel(),
        mgate.sel(),
        0,
        (size / cfg::PAGE_SIZE) as Selector,
        Perm::R,
    )?;
    Ok(mgate)
}

pub fn free_rbuf(rbuf: RecvBuf) {
    BUFS.get_mut().free(rbuf.addr as u64, rbuf.size as u64);
}

pub(crate) fn init() {
    let (addr, size) = VPE::cur().pe_desc().rbuf_space();
    BUFS.set(MemMap::new(addr as u64, size as u64));
}
