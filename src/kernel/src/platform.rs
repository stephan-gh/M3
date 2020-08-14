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

use base::cell::StaticCell;
use base::col::{String, Vec};
use base::kif::{boot, PEDesc};
use base::mem::GlobAddr;
use base::tcu::PEId;
use base::util;
use core::iter;

use crate::arch;

#[cfg(target_os = "none")]
pub use arch::platform::rbuf_pemux;

pub struct KEnv {
    info: boot::Info,
    info_addr: GlobAddr,
    mods: Vec<boot::Mod>,
    pes: Vec<PEDesc>,
}

impl KEnv {
    pub fn new(
        info: boot::Info,
        info_addr: GlobAddr,
        mods: Vec<boot::Mod>,
        pes: Vec<PEDesc>,
    ) -> Self {
        KEnv {
            info,
            info_addr,
            mods,
            pes,
        }
    }
}

pub struct PEIterator {
    id: PEId,
    last: PEId,
}

impl PEIterator {
    pub fn new(id: PEId, last: PEId) -> Self {
        Self { id, last }
    }
}

impl iter::Iterator for PEIterator {
    type Item = PEId;

    fn next(&mut self) -> Option<Self::Item> {
        if self.id <= self.last {
            self.id += 1;
            Some(self.id - 1)
        }
        else {
            None
        }
    }
}

static KENV: StaticCell<Option<KEnv>> = StaticCell::new(None);

pub fn init(args: &[String]) {
    KENV.set(Some(arch::platform::init(args)));
}

fn get() -> &'static mut KEnv {
    KENV.get_mut().as_mut().unwrap()
}

pub fn info() -> &'static boot::Info {
    &get().info
}

pub fn info_addr() -> GlobAddr {
    get().info_addr
}
pub fn info_size() -> usize {
    util::size_of::<boot::Info>()
        + info().mod_count as usize * util::size_of::<boot::Mod>()
        + info().pe_count as usize * util::size_of::<boot::PE>()
        + info().mem_count as usize * util::size_of::<boot::Mem>()
}

pub fn kernel_pe() -> PEId {
    arch::platform::kernel_pe()
}
#[cfg(target_os = "linux")]
pub fn pes() -> &'static [PEDesc] {
    &get().pes
}
pub fn user_pes() -> PEIterator {
    arch::platform::user_pes()
}

pub fn pe_desc(pe: PEId) -> PEDesc {
    get().pes[pe as usize]
}

pub fn is_shared(pe: PEId) -> bool {
    arch::platform::is_shared(pe)
}

pub fn mods() -> &'static [boot::Mod] {
    &get().mods
}
