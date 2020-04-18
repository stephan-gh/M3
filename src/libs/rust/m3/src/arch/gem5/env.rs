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

use base;
use cap::Selector;
use cfg;
use com::{SendGate, SliceSource};
use core::cmp;
use core::intrinsics;
use env;
use kif::{self, PEDesc};
use pes::VPE;
use session::{Pager, ResMng};
use tcu;
use util;
use vfs::{FileTable, MountTable};

#[derive(Default, Copy, Clone)]
#[repr(C, packed)]
pub struct EnvData {
    base: base::envdata::EnvData,
}

impl EnvData {
    pub fn pe_id(&self) -> u64 {
        self.base.pe_id
    }

    pub fn shared(&self) -> bool {
        self.base.shared != 0
    }

    pub fn pe_desc(&self) -> PEDesc {
        PEDesc::new_from(self.base.pe_desc)
    }

    pub fn set_pedesc(&mut self, pe: PEDesc) {
        self.base.pe_desc = pe.value();
    }

    pub fn argc(&self) -> usize {
        self.base.argc as usize
    }

    pub fn set_argc(&mut self, argc: usize) {
        self.base.argc = argc as u32;
    }

    pub fn set_argv(&mut self, argv: usize) {
        self.base.argv = argv as u64;
    }

    pub fn sp(&self) -> usize {
        self.base.sp as usize
    }

    pub fn set_sp(&mut self, sp: usize) {
        self.base.sp = sp as u64;
    }

    pub fn set_entry(&mut self, entry: usize) {
        self.base.entry = entry as u64;
    }

    pub fn heap_size(&self) -> usize {
        self.base.heap_size as usize
    }

    pub fn set_heap_size(&mut self, size: usize) {
        self.base.heap_size = size as u64;
    }

    pub fn first_std_ep(&self) -> tcu::EpId {
        self.base.first_std_ep as tcu::EpId
    }

    pub fn set_first_std_ep(&mut self, start: tcu::EpId) {
        self.base.first_std_ep = start as u64;
    }

    pub fn has_vpe(&self) -> bool {
        self.base.vpe_addr != 0
    }

    pub fn vpe(&self) -> &'static mut VPE {
        // safety: we trust our loader
        unsafe { intrinsics::transmute(self.base.vpe_addr as usize) }
    }

    pub fn load_pager(&self) -> Option<Pager> {
        match self.base.pager_sess {
            0 => None,
            s => Some(Pager::new_bind(s as Selector).unwrap()),
        }
    }

    pub fn load_rmng(&self) -> ResMng {
        ResMng::new(SendGate::new_bind(self.base.rmng_sel as Selector))
    }

    pub fn load_first_sel(&self) -> Selector {
        // it's initially 0. make sure it's at least the first usable selector
        cmp::max(kif::FIRST_FREE_SEL, self.base.first_sel as Selector)
    }

    pub fn load_mounts(&self) -> MountTable {
        if self.base.mounts_len != 0 {
            // safety: we trust our loader
            let slice = unsafe {
                util::slice_for(
                    self.base.mounts_addr as *const u64,
                    self.base.mounts_len as usize,
                )
            };
            MountTable::unserialize(&mut SliceSource::new(slice))
        }
        else {
            MountTable::default()
        }
    }

    pub fn load_fds(&self) -> FileTable {
        if self.base.fds_len != 0 {
            // safety: we trust our loader
            let slice = unsafe {
                util::slice_for(self.base.fds_addr as *const u64, self.base.fds_len as usize)
            };
            FileTable::unserialize(&mut SliceSource::new(slice))
        }
        else {
            FileTable::default()
        }
    }

    // --- gem5 specific API ---

    pub fn set_vpe(&mut self, vpe: &VPE) {
        self.base.vpe_addr = vpe as *const VPE as u64;
    }

    pub fn has_lambda(&self) -> bool {
        self.base.lambda == 1
    }

    pub fn set_lambda(&mut self, lambda: bool) {
        self.base.lambda = lambda as u64;
    }

    pub fn set_first_sel(&mut self, sel: Selector) {
        self.base.first_sel = u64::from(sel);
    }

    pub fn set_rmng(&mut self, sel: Selector) {
        self.base.rmng_sel = u64::from(sel);
    }

    pub fn set_files(&mut self, off: usize, len: usize) {
        self.base.fds_addr = off as u64;
        self.base.fds_len = len as u64;
    }

    pub fn set_mounts(&mut self, off: usize, len: usize) {
        self.base.mounts_addr = off as u64;
        self.base.mounts_len = len as u64;
    }

    pub fn set_pager(&mut self, pager: &Pager) {
        self.base.pager_sess = pager.sel() as u64;
    }
}

pub fn get() -> &'static mut EnvData {
    // safety: we trust our loader
    unsafe { intrinsics::transmute(cfg::ENV_START) }
}

pub fn closure() -> &'static mut env::Closure {
    // safety: we trust our loader
    unsafe { intrinsics::transmute(cfg::ENV_START + util::size_of::<EnvData>()) }
}
