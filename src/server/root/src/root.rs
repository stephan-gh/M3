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

#![no_std]

#[macro_use]
extern crate m3;

use m3::cell::RefCell;
use m3::col::Vec;
use m3::com::MemGate;
use m3::goff;
use m3::kif::{boot, PEDesc};
use m3::rc::Rc;
use m3::util;
use m3::vfs::FileRef;
use m3::vpe::{Activity, VPE, VPEArgs};

mod loader;

#[no_mangle]
pub fn main() -> i32 {
    // TODO don't use hardcoded selector
    let mgate = MemGate::new_bind(1000);
    let mut off: goff = 0;

    let info: boot::Info = mgate.read_obj(0).expect("Unable to read boot info");
    off += util::size_of::<boot::Info>() as goff;

    println!("Found info={:?}", info);

    let mut mods = vec![0u8; info.mod_size as usize];
    mgate.read(&mut mods, off).expect("Unable to read mods");
    off += info.mod_size;

    let moditer = boot::ModIterator::new(mods.as_slice().as_ptr() as usize, info.mod_size as usize);
    for m in moditer {
        println!("{:?}", m);
    }

    let mut pes: Vec<PEDesc> = Vec::with_capacity(info.pe_count as usize);
    unsafe { pes.set_len(info.pe_count as usize) };
    mgate.read(&mut pes, off).expect("Unable to read PEs");

    let mut i = 0;
    for pe in pes {
        println!(
            "PE{:02}: {} {} {} KiB memory",
            i, pe.pe_type(), pe.isa(), pe.mem_size() / 1024
        );
        i += 1;
    }

    let mut bsel = mgate.sel();
    let moditer = boot::ModIterator::new(mods.as_slice().as_ptr() as usize, info.mod_size as usize);
    for m in moditer {
        bsel += 1;
        if m.name() == "rctmux" || m.name() == "root" {
            continue;
        }

        let mut vpe = VPE::new_with(VPEArgs::new(m.name())).expect("Unable to create VPE");
        println!("Boot module {} runs on {:?}", m.name(), vpe.pe());

        let mut bfile = loader::BootFile::new(bsel, m.size as usize);
        let mut bmapper = loader::BootMapper::new(vpe.sel(), bsel, vpe.pe().has_virtmem());
        let bfileref = FileRef::new(Rc::new(RefCell::new(bfile)), 0);
        let act = vpe.exec_file(&mut bmapper, bfileref, &[m.name()]).expect("Unable to exec boot module");

        act.wait().expect("Unable to wait for VPE");
    }

    0
}
