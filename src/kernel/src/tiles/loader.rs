/*
 * Copyright (C) 2020-2022 Nils Asmussen, Barkhausen Institut
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

use base::cfg::{ENV_START, MOD_HEAP_SIZE, PAGE_BITS, PAGE_MASK, PAGE_SIZE};
use base::col::Vec;
use base::elf;
use base::env;
use base::errors::{Code, Error};
use base::goff;
use base::kif::{self, PageFlags};
use base::mem::{size_of, GlobAddr};
use base::tcu;
use base::util::math;

use crate::cap::{Capability, KObject, MapObject, SelRange};
use crate::ktcu;
use crate::mem;
use crate::tiles::{tilemng, Activity, TileMux};

use crate::platform;

pub fn init_memory_async(act: &Activity) -> Result<i32, Error> {
    // put mapping for env into cap table (so that we can access it in create_mgate later)
    let env_phys = if platform::tile_desc(act.tile_id()).has_virtmem() {
        let mut env_addr = TileMux::translate_async(
            tilemng::tilemux(act.tile_id()),
            act.id(),
            ENV_START as goff,
            kif::PageFlags::RW,
        )?;
        env_addr = env_addr + (ENV_START & PAGE_MASK) as goff;

        let flags = PageFlags::from(kif::Perm::RW);
        load_segment_async(act, env_addr, ENV_START as goff, PAGE_SIZE, flags, false)?;

        ktcu::glob_to_phys_remote(act.tile_id(), env_addr, flags)?
    }
    else {
        ENV_START as goff
    };

    if act.is_root() {
        load_root_async(env_phys, act)?;
    }
    Ok(0)
}

fn load_root_async(env_phys: goff, act: &Activity) -> Result<(), Error> {
    // map stack
    if act.tile_desc().has_virtmem() {
        let (virt, size) = act.tile_desc().stack_space();
        let phys =
            mem::borrow_mut().allocate(mem::MemType::ROOT, size as goff, PAGE_SIZE as goff)?;
        load_segment_async(act, phys.global(), virt as goff, size, PageFlags::RW, true)?;
    }

    let entry: usize = {
        let app = get_mod("root").ok_or_else(|| Error::new(Code::NoSuchFile))?;
        klog!(ACTIVITIES, "Loading mod '{}':", app.name());
        load_mod_async(act, app)?
    };

    let argv_addr = write_arguments(env_phys, act.tile_id(), &["root"]);

    // build env
    let mut senv = env::BaseEnv {
        boot: env::BootEnv {
            platform: env::boot().platform,
            argc: 1,
            argv: argv_addr as u64,
            tile_id: act.tile_id().raw() as u64,
            tile_desc: act.tile_desc().value(),
            ..Default::default()
        },
        sp: act.tile_desc().stack_top() as u64,
        entry: entry as u64,
        act_id: act.id() as u64,
        heap_size: MOD_HEAP_SIZE as u64,
        rmng_sel: kif::INVALID_SEL,
        first_sel: act.first_sel(),
        first_std_ep: act.eps_start() as u64,
        ..Default::default()
    };
    let tile_ids = &env::boot().raw_tile_ids[0..env::boot().raw_tile_count as usize];
    senv.boot.raw_tile_count = tile_ids.len() as u64;
    senv.boot.raw_tile_ids[0..tile_ids.len()].copy_from_slice(tile_ids);

    // write env to target tile
    ktcu::write_slice(act.tile_id(), env_phys, &[senv]);
    Ok(())
}

fn get_mod(name: &str) -> Option<&kif::boot::Mod> {
    for m in platform::mods() {
        if let Some(bin_name) = m.name().split(' ').next() {
            if bin_name == name {
                return Some(m);
            }
        }
    }

    None
}

fn read_from_mod<T: Default>(bm: &kif::boot::Mod, off: goff) -> Result<T, Error> {
    if off + size_of::<T>() as goff > bm.size {
        return Err(Error::new(Code::InvalidElf));
    }

    let gaddr = GlobAddr::new(bm.addr);
    Ok(ktcu::read_obj(gaddr.tile(), gaddr.offset() + off))
}

fn load_segment_async(
    act: &Activity,
    phys: GlobAddr,
    virt: goff,
    size: usize,
    flags: PageFlags,
    map: bool,
) -> Result<(), Error> {
    if act.tile_desc().has_virtmem() {
        let dst_sel = virt >> PAGE_BITS;
        let pages = math::round_up(size, PAGE_SIZE) >> PAGE_BITS;

        let phys_align = GlobAddr::new_with(phys.tile(), phys.offset() & !PAGE_MASK as goff);
        let map_obj = MapObject::new(phys_align, flags);
        if map {
            map_obj.map_async(act, virt & !PAGE_MASK as goff, phys_align, pages, flags)?;
        }

        act.map_caps().borrow_mut().insert(Capability::new_range(
            SelRange::new_range(dst_sel as kif::CapSel, pages as kif::CapSel),
            KObject::Map(map_obj),
        ))
    }
    else {
        ktcu::copy(
            // destination
            act.tile_id(),
            virt as goff,
            // source
            phys.tile(),
            phys.offset(),
            size,
        )
    }
}

fn load_mod_async(act: &Activity, bm: &kif::boot::Mod) -> Result<usize, Error> {
    let mod_addr = GlobAddr::new(bm.addr);
    let hdr: elf::ElfHeader = read_from_mod(bm, 0)?;

    if hdr.ident[0] != b'\x7F'
        || hdr.ident[1] != b'E'
        || hdr.ident[2] != b'L'
        || hdr.ident[3] != b'F'
    {
        return Err(Error::new(Code::InvalidElf));
    }

    // copy load segments to destination tile
    let mut end = 0;
    let mut off = hdr.ph_off;
    for _ in 0..hdr.ph_num {
        // load program header
        let phdr: elf::ProgramHeader = read_from_mod(bm, off as goff)?;
        off += hdr.ph_entry_size as usize;

        // we're only interested in non-empty load segments
        if phdr.ty != elf::PHType::LOAD.val || phdr.mem_size == 0 {
            continue;
        }

        let flags = PageFlags::from(kif::Perm::from(elf::PHFlags::from_bits_truncate(
            phdr.flags,
        )));
        let offset = math::round_dn(phdr.offset as usize, PAGE_SIZE);
        let virt = math::round_dn(phdr.virt_addr, PAGE_SIZE);

        // bss?
        if phdr.file_size == 0 {
            let size = math::round_up(
                (phdr.virt_addr & PAGE_MASK) + phdr.mem_size as usize,
                PAGE_SIZE,
            );

            let phys = if act.tile_desc().has_virtmem() {
                let mem = mem::borrow_mut().allocate(
                    mem::MemType::ROOT,
                    size as goff,
                    PAGE_SIZE as goff,
                )?;
                load_segment_async(act, mem.global(), virt as goff, size, flags, true)?;
                ktcu::glob_to_phys_remote(act.tile_id(), mem.global(), flags)?
            }
            else {
                virt as goff
            };

            ktcu::clear(act.tile_id(), phys, size)?;
            end = virt + size;
        }
        else {
            assert!(phdr.mem_size == phdr.file_size);
            let size = (phdr.offset as usize & PAGE_MASK) + phdr.file_size as usize;
            load_segment_async(
                act,
                mod_addr + offset as goff,
                virt as goff,
                size,
                flags,
                true,
            )?;
            end = virt + size;
        }
    }

    if act.tile_desc().has_virtmem() {
        // create initial heap
        let end = math::round_up(end, PAGE_SIZE);
        let phys = mem::borrow_mut().allocate(
            mem::MemType::ROOT,
            MOD_HEAP_SIZE as goff,
            PAGE_SIZE as goff,
        )?;
        load_segment_async(
            act,
            phys.global(),
            end as goff,
            MOD_HEAP_SIZE,
            PageFlags::RW,
            true,
        )?;
    }

    Ok(hdr.entry)
}

fn write_arguments(addr: goff, tile: tcu::TileId, args: &[&str]) -> usize {
    let mut argptr: Vec<u64> = Vec::new();
    let mut argbuf: Vec<u8> = Vec::new();

    let off = addr + size_of::<env::BaseEnv>() as goff;
    let mut argoff = ENV_START + size_of::<env::BaseEnv>();
    for s in args {
        // push argv entry
        argptr.push(argoff as u64);

        // push string
        let arg = s.as_bytes();
        argbuf.extend_from_slice(arg);

        // 0-terminate it
        argbuf.push(b'\0');

        argoff += arg.len() + 1;
    }

    ktcu::write_mem(
        tile,
        off as goff,
        argbuf.as_ptr() as *const u8,
        argbuf.len(),
    );
    let argv_size = argptr.len() * size_of::<u64>();
    argoff = math::round_up(argoff, size_of::<u64>());
    ktcu::write_mem(
        tile,
        addr + (argoff - ENV_START) as goff,
        argptr.as_ptr() as *const u8,
        argv_size,
    );
    argoff
}
