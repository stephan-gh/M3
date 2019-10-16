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

use arch;
use base;
use cap::Selector;
use cell::StaticCell;
use col::{String, Vec};
use com::{SendGate, SliceSource};
use core::intrinsics;
use dtu::{EpId, Label};
use kif::{self, PEDesc, PEType, PEISA};
use libc;
use pes::VPE;
use session::{Pager, ResMng};
use vfs::{FileTable, MountTable};

pub struct EnvData {
    sysc_crd: u64,
    sysc_lbl: Label,
    sysc_ep: EpId,
    _shm_prefix: String,

    vpe: usize,
}

impl EnvData {
    fn base(&self) -> &base::envdata::EnvData {
        base::envdata::get()
    }

    pub fn pe_id(&self) -> u32 {
        self.base().pe_id
    }

    pub fn shared(&self) -> bool {
        self.base().shared != 0
    }

    pub fn pe_desc(&self) -> PEDesc {
        PEDesc::new_from(self.base().pe_desc)
    }

    pub fn argc(&self) -> usize {
        self.base().argc as usize
    }

    pub fn argv(&self) -> *const *const i8 {
        self.base().argv as *const *const i8
    }

    pub fn has_vpe(&self) -> bool {
        self.vpe != 0
    }

    pub fn vpe(&self) -> &'static mut VPE {
        unsafe { intrinsics::transmute(self.vpe) }
    }

    pub fn set_vpe(&mut self, vpe: &VPE) {
        self.vpe = vpe as *const VPE as usize;
    }

    pub fn load_pager(&self) -> Option<Pager> {
        None
    }

    pub fn load_rmng(&self) -> ResMng {
        ResMng::new(SendGate::new_bind(Self::load_word("rmng", 0) as Selector))
    }

    pub fn load_eps(&self) -> u64 {
        Self::load_word("eps", 0)
    }

    pub fn load_nextsel(&self) -> Selector {
        if self.base().first_sel != 0 {
            self.base().first_sel as Selector
        }
        else {
            Self::load_word("nextsel", u64::from(kif::FIRST_FREE_SEL)) as Selector
        }
    }

    pub fn load_rbufs(&self) -> arch::rbufs::RBufSpace {
        match arch::loader::read_env_file("rbufs") {
            Some(rbuf) => {
                let mut ss = SliceSource::new(&rbuf);
                arch::rbufs::RBufSpace::new_with(ss.pop(), ss.pop())
            },
            None => arch::rbufs::RBufSpace::new(),
        }
    }

    pub fn load_mounts(&self) -> MountTable {
        match arch::loader::read_env_file("ms") {
            Some(ms) => MountTable::unserialize(&mut SliceSource::new(&ms)),
            None => MountTable::default(),
        }
    }

    pub fn load_fds(&self) -> FileTable {
        match arch::loader::read_env_file("fds") {
            Some(fds) => FileTable::unserialize(&mut SliceSource::new(&fds)),
            None => FileTable::default(),
        }
    }

    fn load_word(name: &str, default: u64) -> u64 {
        match arch::loader::read_env_file(name) {
            Some(buf) => SliceSource::new(&buf).pop(),
            None => default,
        }
    }

    // --- host specific API ---

    pub fn syscall_params(&self) -> (EpId, Label, u64) {
        (self.sysc_ep, self.sysc_lbl, self.sysc_crd)
    }
}

fn read_line(fd: i32) -> String {
    let mut vec = Vec::new();
    loop {
        let mut buf = [0u8; 1];
        if unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, 1) } == 0 {
            break;
        }
        if buf[0] == b'\n' {
            break;
        }
        vec.push(buf[0]);
    }
    unsafe { String::from_utf8_unchecked(vec) }
}

static ENV_DATA: StaticCell<Option<EnvData>> = StaticCell::new(None);

pub fn get() -> &'static mut EnvData {
    ENV_DATA.get_mut().as_mut().unwrap()
}

pub fn init(argc: i32, argv: *const *const i8) {
    let fd = unsafe {
        let path = format!("/tmp/m3/{}\0", libc::getpid());
        libc::open(path.as_ptr() as *const libc::c_char, libc::O_RDONLY)
    };
    assert!(fd != -1);

    let shm_prefix = read_line(fd);

    let base = base::envdata::EnvData::new(
        read_line(fd).parse::<u32>().unwrap(),
        PEDesc::new(PEType::COMP_IMEM, PEISA::X86, 1024 * 1024),
        argc,
        argv,
        read_line(fd).parse::<Selector>().unwrap(),
        read_line(fd).parse::<Selector>().unwrap(),
    );
    base::envdata::set(base);

    ENV_DATA.set(Some(EnvData {
        sysc_lbl: read_line(fd).parse::<Label>().unwrap(),
        sysc_ep: read_line(fd).parse::<EpId>().unwrap(),
        sysc_crd: read_line(fd).parse::<u64>().unwrap(),
        _shm_prefix: shm_prefix,

        vpe: 0,
    }));

    unsafe {
        libc::close(fd);
    }
}

pub fn reinit() {
    init(get().argc() as i32, get().argv());
}
