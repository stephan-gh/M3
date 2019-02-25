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

use m3::cap::Selector;
use m3::cell::RefCell;
use m3::col::{String, ToString, Vec};
use m3::com::MemGate;
use m3::goff;
use m3::kif::{boot, PEDesc};
use m3::rc::Rc;
use m3::syscalls;
use m3::util;
use m3::vfs::FileRef;
use m3::vpe::{Activity, ExecActivity, VPE, VPEArgs};

mod loader;

pub struct Child {
    name: String,
    args: Vec<String>,
    reqs: Vec<String>,
    daemon: bool,
    activity: ExecActivity,
    mapper: loader::BootMapper,
}

impl Child {
    pub fn new(name: String, args: Vec<String>, reqs: Vec<String>, daemon: bool,
               activity: ExecActivity, mapper: loader::BootMapper) -> Self {
        Child {
            name: name,
            args: args,
            reqs: reqs,
            daemon: daemon,
            activity: activity,
            mapper: mapper,
        }
    }
}

fn remove_by_sel(vpes: &mut Vec<Child>, sel: Selector) -> Option<Child> {
    let idx = vpes.iter().position(|c| c.activity.vpe().sel() == sel);
    if let Some(i) = idx {
        let child = vpes.remove(i);
        return Some(child);
    }
    else {
        None
    }
}

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

    let mut childs = Vec::<Child>::new();

    let mut bsel = mgate.sel();
    let moditer = boot::ModIterator::new(mods.as_slice().as_ptr() as usize, info.mod_size as usize);
    for m in moditer {
        bsel += 1;
        if m.name() == "rctmux" || m.name() == "root" {
            continue;
        }

        let mut args = Vec::<String>::new();
        let mut reqs = Vec::<String>::new();
        let mut name: String = String::new();
        let mut daemon = false;
        for (idx, a) in m.name().split_whitespace().enumerate() {
            if idx == 0 {
                name = a.to_string();
            }
            else {
                if a.starts_with("requires=") {
                    reqs.push(a.to_string());
                }
                else if a == "daemon" {
                    daemon = true;
                }
                else {
                    args.push(a.to_string());
                }
            }
        }

        let mut vpe = VPE::new_with(VPEArgs::new(&name)).expect("Unable to create VPE");
        println!("Boot module '{}' runs on {:?}", name, vpe.pe());

        let mut bfile = loader::BootFile::new(bsel, m.size as usize);
        let mut bmapper = loader::BootMapper::new(vpe.sel(), bsel, vpe.pe().has_virtmem());
        let bfileref = FileRef::new(Rc::new(RefCell::new(bfile)), 0);
        let act = vpe.exec_file(&mut bmapper, bfileref, &args).expect("Unable to exec boot module");

        childs.push(Child::new(name, args, reqs, daemon, act, bmapper));
    }

    while childs.len() > 0 {
        let mut sels = Vec::new();
        for c in &childs {
            sels.push(c.activity.vpe().sel());
        }

        let (sel, code) = syscalls::vpe_wait(&sels).expect("Unable to wait for VPEs");
        let child = assert_some!(remove_by_sel(&mut childs, sel));
        println!("Child '{}' exited with exitcode {}", child.name, code);
    }

    0
}
