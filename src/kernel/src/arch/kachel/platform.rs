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
use base::cfg;
use base::tcu::PEId;
use base::envdata;
use base::goff;
use base::kif::{boot, PEDesc, PEType};
use base::mem::GlobAddr;
use base::util;

use args;
use ktcu;
use mem::{self, MemMod, MemType};
use platform;

static LAST_PE: StaticCell<PEId> = StaticCell::new(0);

pub fn init(_args: &[String]) -> platform::KEnv {
    // read kernel env
    let addr = GlobAddr::new(envdata::get().kenv);
    let mut offset = addr.offset();
    let info: boot::Info = ktcu::read_obj(addr.pe(), offset);
    offset += util::size_of::<boot::Info>() as goff;

    // read boot modules
    let mut mods: Vec<boot::Mod> = Vec::with_capacity(info.mod_count as usize);
    unsafe { mods.set_len(info.mod_count as usize) };
    ktcu::read_slice(addr.pe(), offset, &mut mods);
    offset += info.mod_count as goff * util::size_of::<boot::Mod>() as goff;

    // read PEs
    let mut pes: Vec<PEDesc> = Vec::with_capacity(info.pe_count as usize);
    unsafe { pes.set_len(info.pe_count as usize) };
    ktcu::read_slice(addr.pe(), offset, &mut pes);
    offset += info.pe_count as goff * util::size_of::<PEDesc>() as goff;

    // read memory regions
    let mut mems: Vec<boot::Mem> = Vec::with_capacity(info.mem_count as usize);
    unsafe { mems.set_len(info.mem_count as usize) };
    ktcu::read_slice(addr.pe(), offset, &mut mems);

    // build new info for user PEs
    let mut uinfo = boot::Info::default();
    uinfo.mod_count = info.mod_count;
    uinfo.pe_count = info.pe_count;
    uinfo.mem_count = info.mem_count;
    uinfo.serv_count = 0;

    let mut umems = Vec::new();
    let mut upes = Vec::new();

    // register memory modules
    let mut kmem_idx = 0;
    let mem: &mut mem::MainMemory = mem::get();
    for (i, pe) in pes.iter().enumerate() {
        if pe.pe_type() == PEType::MEM {
            // the first memory module hosts the FS image and other stuff
            if kmem_idx == 0 {
                let avail = mems[kmem_idx].size();
                if avail <= args::get().kmem as goff {
                    panic!("Not enough DRAM for kernel memory ({})", args::get().kmem);
                }

                // file system image
                let used = pe.mem_size() as goff - avail;
                mem.add(MemMod::new(MemType::OCCUPIED, i as PEId, 0, used));
                umems.push(boot::Mem::new(0, used, true));

                // kernel memory
                mem.add(MemMod::new(
                    MemType::KERNEL,
                    i as PEId,
                    used,
                    args::get().kmem as goff,
                ));

                // user memory
                let user = used + args::get().kmem as goff;
                mem.add(MemMod::new(MemType::USER, i as PEId, user, avail));
                umems.push(boot::Mem::new(user, avail - args::get().kmem as goff, false));
            }
            else {
                mem.add(MemMod::new(MemType::USER, i as PEId, 0, pe.mem_size() as goff));
                umems.push(boot::Mem::new(0, pe.mem_size() as goff, false));
            }
            kmem_idx += 1;
        }
        else {
            if kmem_idx > 0 {
                panic!("All memory PEs have to be last");
            }

            LAST_PE.set(i as PEId);

            if i > 0 {
                assert!(kernel_pe() == 0);
                upes.push(boot::PE::new(i as u32, *pe));
            }
        }
    }

    // write-back boot info
    let mut uoffset = addr.offset();
    uinfo.pe_count = upes.len() as u64;
    uinfo.mem_count = umems.len() as u64;
    ktcu::write_slice(addr.pe(), uoffset, &[uinfo]);
    uoffset += util::size_of::<boot::Info>() as goff;
    uoffset += info.mod_count as goff * util::size_of::<boot::Mod>() as goff;

    // write-back user PEs
    ktcu::write_slice(addr.pe(), uoffset, &upes);
    uoffset += uinfo.pe_count as goff * util::size_of::<boot::PE>() as goff;

    // write-back user memory regions
    ktcu::write_slice(addr.pe(), uoffset, &umems);

    platform::KEnv::new(info, addr, mods, pes)
}

pub fn kernel_pe() -> PEId {
    envdata::get().pe_id as PEId
}
pub fn user_pes() -> platform::PEIterator {
    platform::PEIterator::new(kernel_pe() + 1, *LAST_PE)
}

pub fn is_shared(pe: PEId) -> bool {
    platform::pe_desc(pe).is_programmable()
}

pub fn rbuf_pemux(pe: PEId) -> goff {
    match platform::pe_desc(pe).has_virtmem() {
        true => cfg::PEMUX_RBUF_PHYS as goff,
        false => cfg::PEMUX_RBUF_SPACE as goff,
    }
}
