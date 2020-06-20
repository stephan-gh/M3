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

use base::cfg;
use base::col::{String, Vec};
use base::goff;
use base::kif::{boot, PEDesc, PEType, PEISA};
use base::libc;
use base::tcu::PEId;
use base::util;
use core::mem::MaybeUninit;
use core::ptr;

use args;
use mem;
use platform;

pub fn init(args: &[String]) -> platform::KEnv {
    let mut info = boot::Info::default();

    // PEs
    let mut pes = Vec::new();
    for _ in 0..cfg::PE_COUNT {
        pes.push(PEDesc::new(PEType::COMP_IMEM, PEISA::X86, 1024 * 1024));
    }
    if args::get().disk {
        pes.push(PEDesc::new(PEType::COMP_IMEM, PEISA::IDE_DEV, 0));
    }
    if args::get().net_bridge.is_some() {
        pes.push(PEDesc::new(PEType::COMP_IMEM, PEISA::NIC_DEV, 0));
        pes.push(PEDesc::new(PEType::COMP_IMEM, PEISA::NIC_DEV, 0));
    }
    let mut upes = Vec::new();
    for (i, pe) in pes[1..].iter().enumerate() {
        upes.push(boot::PE::new(i as u32, *pe));
    }
    info.pe_count = upes.len() as u64;

    let mems = build_mems();
    info.mem_count = mems.len() as u64;

    let mods = build_modules(args);
    info.mod_count = mods.len() as u64;

    // build kinfo page
    let bsize = util::size_of::<boot::Info>()
        + info.mod_count as usize * util::size_of::<boot::Mod>()
        + info.pe_count as usize * util::size_of::<boot::PE>()
        + info.mem_count as usize * util::size_of::<boot::Mem>();
    let mut binfo_mem = mem::get()
        .allocate(bsize as goff, 1)
        .expect("Unable to allocate mem for boot info");

    unsafe {
        // info
        let mut dest = binfo_mem.global().offset();
        libc::memcpy(
            dest as *mut u8 as *mut libc::c_void,
            &info as *const boot::Info as *const libc::c_void,
            util::size_of::<boot::Info>(),
        );
        dest += util::size_of::<boot::Info>() as goff;

        // modules
        libc::memcpy(
            dest as *mut u8 as *mut libc::c_void,
            mods.as_ptr() as *const libc::c_void,
            mods.len() * util::size_of::<boot::Mod>(),
        );
        dest += (mods.len() * util::size_of::<boot::Mod>()) as goff;

        // PEs
        libc::memcpy(
            dest as *mut u8 as *mut libc::c_void,
            upes.as_ptr() as *const libc::c_void,
            upes.len() * util::size_of::<boot::PE>(),
        );
        dest += (upes.len() * util::size_of::<boot::PE>()) as goff;

        // memories
        libc::memcpy(
            dest as *mut u8 as *mut libc::c_void,
            mems.as_ptr() as *const libc::c_void,
            mems.len() * util::size_of::<boot::Mem>(),
        );
    }
    binfo_mem.claim();

    platform::KEnv::new(info, binfo_mem.global(), mods, pes)
}

fn build_mems() -> Vec<boot::Mem> {
    // create memory
    let base = unsafe {
        libc::mmap(
            ptr::null_mut(),
            cfg::TOTAL_MEM_SIZE,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_ANON | libc::MAP_PRIVATE,
            -1,
            0,
        )
    };
    assert!(base != libc::MAP_FAILED);
    let mut off = base as goff;

    // fs image
    mem::get().add(mem::MemMod::new(
        mem::MemType::OCCUPIED,
        kernel_pe(),
        off,
        cfg::FS_MAX_SIZE as goff,
    ));
    off += cfg::FS_MAX_SIZE as goff;

    // kernel memory
    mem::get().add(mem::MemMod::new(
        mem::MemType::KERNEL,
        kernel_pe(),
        off,
        args::get().kmem as goff,
    ));
    off += args::get().kmem as goff;

    // user memory
    let user_size = cfg::TOTAL_MEM_SIZE - (cfg::FS_MAX_SIZE + args::get().kmem);
    mem::get().add(mem::MemMod::new(
        mem::MemType::USER,
        kernel_pe(),
        off,
        user_size as goff,
    ));

    // set memories
    let mut mems = Vec::new();
    mems.push(boot::Mem::new(0, cfg::FS_MAX_SIZE as goff, true));
    mems.push(boot::Mem::new(off, user_size as goff, false));
    mems
}

fn build_modules(args: &[String]) -> Vec<boot::Mod> {
    let mut mods = Vec::new();

    for arg in args {
        // copy boot module into memory
        unsafe {
            let fd = libc::open(arg.as_ptr() as *const libc::c_char, libc::O_RDONLY);
            if fd == -1 {
                panic!("Opening {} for reading failed", arg);
            }
            let mut finfo: libc::stat = MaybeUninit::uninit().assume_init();
            if libc::fstat(fd, &mut finfo) == -1 {
                panic!("Stat for {} failed", arg);
            }

            let mut alloc = mem::get()
                .allocate(finfo.st_size as goff, 1)
                .expect("Unable to alloc mem for boot module");
            let dest = alloc.global().offset() as *mut u8 as *mut libc::c_void;
            if libc::read(fd, dest, alloc.size() as usize) == -1 {
                panic!("Reading from {} failed", arg);
            }
            libc::close(fd);

            let mod_name = arg.rsplitn(2, '/').next().unwrap();
            mods.push(boot::Mod::new(alloc.global().raw(), alloc.size(), mod_name));

            // don't free mem
            alloc.claim();
        }
    }

    mods
}

pub fn kernel_pe() -> PEId {
    0
}
pub fn user_pes() -> platform::PEIterator {
    platform::PEIterator::new(1, platform::pes().len() - 1)
}

pub fn is_shared(_pe: PEId) -> bool {
    false
}

pub fn rbuf_pemux(_pe: PEId) -> goff {
    0
}
