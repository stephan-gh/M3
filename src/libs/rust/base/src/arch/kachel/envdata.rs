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

use core::intrinsics;

use crate::cfg;

#[derive(Default, Copy, Clone)]
#[repr(C)]
pub struct EnvData {
    // boot env
    pub platform: u64,
    pub pe_id: u64,
    pub pe_desc: u64,
    pub argc: u64,
    pub argv: u64,
    pub heap_size: u64,
    pub pe_mem_base: u64,
    pub pe_mem_size: u64,
    pub kenv: u64,
    pub lambda: u64,

    // set by PEMux
    pub shared: u64,

    // m3 env
    pub sp: u64,
    pub entry: u64,
    pub first_std_ep: u64,
    pub first_sel: u64,

    pub rmng_sel: u64,
    pub pager_sess: u64,

    pub mounts_addr: u64,
    pub mounts_len: u64,

    pub fds_addr: u64,
    pub fds_len: u64,

    pub vpe_addr: u64,
    pub backend_addr: u64,
}

pub fn get() -> &'static EnvData {
    // safety: the cast is okay because we trust our loader to put the environment at that place
    unsafe { intrinsics::transmute(cfg::ENV_START) }
}
