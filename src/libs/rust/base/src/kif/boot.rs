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

//! The boot information that the kernel passes to root

use core::fmt;

use crate::kif;
use crate::mem::{GlobAddr, MaybeUninit};
use crate::util;

const MAX_MODNAME_LEN: usize = 64;
const MAX_SERVNAME_LEN: usize = 32;

/// The boot information
#[repr(C, packed)]
#[derive(Default, Copy, Clone, Debug)]
pub struct Info {
    /// The number of boot modules
    pub mod_count: u64,
    /// The number of PEs
    pub pe_count: u64,
    /// The number of memory regions
    pub mem_count: u64,
    /// The number of services
    pub serv_count: u64,
}

/// A boot module
#[repr(C, packed)]
pub struct Mod {
    /// The global address of the module
    pub addr: u64,
    /// The size of the module
    pub size: u64,
    name: [i8; MAX_MODNAME_LEN],
}

impl Mod {
    /// Creates a new boot module
    pub fn new(addr: GlobAddr, size: u64, name: &str) -> Self {
        assert!(name.len() < MAX_MODNAME_LEN);
        #[allow(clippy::uninit_assumed_init)]
        let mut m = Self {
            addr: addr.raw(),
            size,
            name: unsafe { MaybeUninit::uninit().assume_init() },
        };
        for (a, c) in m.name.iter_mut().zip(name.bytes()) {
            *a = c as i8;
        }
        m.name[name.len()] = 0;
        m
    }

    /// Returns the global address of the module
    pub fn addr(&self) -> GlobAddr {
        GlobAddr::new(self.addr)
    }

    /// Returns the name and arguments of the module
    pub fn name(&self) -> &'static str {
        // safety: we trust our loader
        unsafe { util::cstr_to_str(self.name.as_ptr()) }
    }
}

impl fmt::Debug for Mod {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(
            f,
            "Mod[addr: {:?}, size: {:#x}, name: {}]",
            self.addr(),
            { self.size },
            self.name()
        )
    }
}

/// A processing element
#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct PE {
    pub id: u32,
    pub desc: kif::PEDesc,
}

impl PE {
    pub fn new(id: u32, desc: kif::PEDesc) -> Self {
        Self { id, desc }
    }
}

impl fmt::Debug for PE {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(
            f,
            "PE{:02}: {} {} {:?} {} KiB memory",
            { self.id },
            self.desc.pe_type(),
            self.desc.isa(),
            self.desc.attr(),
            self.desc.mem_size() / 1024
        )
    }
}

/// A memory region
#[repr(C, packed)]
#[derive(Default, Copy, Clone)]
pub struct Mem {
    addr: u64,
    size: u64,
}

impl Mem {
    /// Creates a new memory region of given size.
    pub fn new(addr: GlobAddr, size: u64, reserved: bool) -> Self {
        assert!((size & 1) == 0);
        Mem {
            addr: addr.raw(),
            size: size | (reserved as u64),
        }
    }

    /// Returns the global address of this memory region
    pub fn addr(&self) -> GlobAddr {
        GlobAddr::new(self.addr)
    }

    /// Returns the size of the memory region
    pub fn size(self) -> u64 {
        self.size & !1
    }

    /// Returns true if the region is reserved, that is, not usable by applications
    pub fn reserved(self) -> bool {
        (self.size & 1) == 1
    }
}

impl fmt::Debug for Mem {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(
            f,
            "Mem[addr: {:?}, size: {:#x}, res={}]",
            self.addr(),
            self.size(),
            self.reserved()
        )
    }
}

/// A service with a certain number of sessions to create
#[repr(C, packed)]
#[derive(Default, Copy, Clone)]
pub struct Service {
    sessions: u32,
    name: [i8; MAX_SERVNAME_LEN],
}

impl Service {
    /// Creates a new service
    pub fn new(name: &str, sessions: u32) -> Self {
        assert!(name.len() < MAX_SERVNAME_LEN);
        #[allow(clippy::uninit_assumed_init)]
        let mut m = Self {
            sessions,
            name: unsafe { MaybeUninit::uninit().assume_init() },
        };
        for (a, c) in m.name.iter_mut().zip(name.bytes()) {
            *a = c as i8;
        }
        m.name[name.len()] = 0;
        m
    }

    pub fn sessions(&self) -> u32 {
        self.sessions
    }

    /// Returns the name of the service
    pub fn name(&self) -> &'static str {
        // safety: we trust our loader
        unsafe { util::cstr_to_str(self.name.as_ptr()) }
    }
}

impl fmt::Debug for Service {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "Serv[name: {}]", self.name(),)
    }
}
