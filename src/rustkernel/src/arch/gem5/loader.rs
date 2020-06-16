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

use base::cell::{Cell, StaticCell};
use base::cfg::{
    ENV_START, LPAGE_SIZE, MOD_HEAP_SIZE, PAGE_BITS, PAGE_MASK, PAGE_SIZE, STACK_BOTTOM, STACK_TOP,
};
use base::col::Vec;
use base::elf;
use base::envdata;
use base::errors::{Code, Error};
use base::goff;
use base::kif::{self, PageFlags};
use base::math;
use base::mem::GlobAddr;
use base::tcu;
use base::util;

use cap::{Capability, KObject, MapObject, SelRange};
use ktcu;
use mem;
use pes::{pemng, VPE};
use platform;

pub struct Loader {
    loaded: Cell<u64>,
}

static LOADER: StaticCell<Option<Loader>> = StaticCell::new(None);

pub fn init() {
    LOADER.set(Some(Loader {
        loaded: Cell::new(0),
    }));
}

impl Loader {
    pub fn get() -> &'static mut Loader {
        LOADER.get_mut().as_mut().unwrap()
    }

    pub fn init_memory(&mut self, vpe: &VPE) -> Result<i32, Error> {
        // put mapping for env into cap table (so that we can access it in create_mgate later)
        let env_addr = if platform::pe_desc(vpe.pe_id()).has_virtmem() {
            let pemux = pemng::get().pemux(vpe.pe_id());
            let env_addr = pemux.translate(vpe.id(), ENV_START as goff, kif::Perm::RW)?;
            let flags = PageFlags::from(kif::Perm::RW);
            Self::load_segment(vpe, env_addr, ENV_START as goff, PAGE_SIZE, flags, false)?;
            env_addr
        }
        else {
            GlobAddr::new(ENV_START as goff)
        };

        if vpe.is_root() {
            self.load_root(env_addr, vpe)?;
        }
        Ok(0)
    }

    pub fn start(&mut self, _vpe: &VPE) -> Result<i32, Error> {
        // nothing to do
        Ok(0)
    }

    pub fn finish_start(&self, _vpe: &VPE) -> Result<(), Error> {
        // nothing to do
        Ok(())
    }

    fn load_root(&mut self, env_addr: GlobAddr, vpe: &VPE) -> Result<(), Error> {
        // map stack
        if vpe.pe_desc().has_virtmem() {
            let virt = STACK_BOTTOM;
            let size = STACK_TOP - virt;
            let mut phys = mem::get().allocate(size as goff, PAGE_SIZE as goff)?;
            Self::load_segment(vpe, phys.global(), virt as goff, size, PageFlags::RW, true)?;
            phys.claim();
        }

        let entry: usize = {
            let (app, first) = self.get_mod("root").ok_or(Error::new(Code::NoSuchFile))?;
            klog!(VPES, "Loading mod '{}':", app.name());
            self.load_mod(vpe, app, !first)?
        };

        let argv_addr = Self::write_arguments(env_addr.raw(), vpe.pe_id(), &["root"]);

        // build env
        let mut senv = envdata::EnvData::default();
        senv.argc = 1;
        senv.argv = argv_addr as u64;
        senv.sp = STACK_TOP as u64;
        senv.entry = entry as u64;
        senv.pe_desc = vpe.pe_desc().value();
        senv.heap_size = MOD_HEAP_SIZE as u64;
        senv.rmng_sel = kif::INVALID_SEL as u64;
        senv.first_sel = vpe.first_sel() as u64;
        senv.first_std_ep = vpe.eps_start() as u64;

        // write env to target PE
        ktcu::write_slice(vpe.pe_id(), env_addr.raw(), &[senv]);
        Ok(())
    }

    fn get_mod(&self, name: &str) -> Option<(&kif::boot::Mod, bool)> {
        for (i, ref m) in platform::mods().iter().enumerate() {
            if let Some(bin_name) = m.name().splitn(2, ' ').next() {
                if bin_name == name {
                    let first = (self.loaded.get() & (1 << i)) == 0;
                    self.loaded.set(self.loaded.get() | 1 << i);
                    return Some((m, first));
                }
            }
        }

        None
    }

    fn read_from_mod<T>(bm: &kif::boot::Mod, off: goff) -> Result<T, Error> {
        if off + util::size_of::<T>() as goff > bm.size {
            return Err(Error::new(Code::InvalidElf));
        }

        let gaddr = GlobAddr::new(bm.addr);
        Ok(ktcu::read_obj(gaddr.pe(), gaddr.offset() + off))
    }

    fn load_segment(
        vpe: &VPE,
        phys: GlobAddr,
        virt: goff,
        size: usize,
        flags: PageFlags,
        map: bool,
    ) -> Result<(), Error> {
        if vpe.pe_desc().has_virtmem() {
            let dst_sel = virt >> PAGE_BITS;
            let pages = math::round_up(size, PAGE_SIZE) >> PAGE_BITS;

            let map_obj = MapObject::new(phys, flags);
            if map {
                map_obj.map(vpe, virt, phys, pages, flags)?;
            }

            vpe.map_caps().borrow_mut().insert(Capability::new_range(
                SelRange::new_range(dst_sel as kif::CapSel, pages as kif::CapSel),
                KObject::Map(map_obj),
            ));
            Ok(())
        }
        else {
            ktcu::copy(
                // destination
                vpe.pe_id(),
                virt as goff,
                // source
                phys.pe(),
                phys.offset(),
                size,
            )
        }
    }

    fn load_mod(&self, vpe: &VPE, bm: &kif::boot::Mod, copy: bool) -> Result<usize, Error> {
        let mod_addr = GlobAddr::new(bm.addr);
        let hdr: elf::Ehdr = Self::read_from_mod(bm, 0)?;

        if hdr.ident[0] != '\x7F' as u8
            || hdr.ident[1] != 'E' as u8
            || hdr.ident[2] != 'L' as u8
            || hdr.ident[3] != 'F' as u8
        {
            return Err(Error::new(Code::InvalidElf));
        }

        // copy load segments to destination PE
        let mut end = 0;
        let mut off = hdr.phoff;
        for _ in 0..hdr.phnum {
            // load program header
            let phdr: elf::Phdr = Self::read_from_mod(bm, off as goff)?;
            off += hdr.phentsize as usize;

            // we're only interested in non-empty load segments
            if phdr.ty != elf::PT::LOAD.val || phdr.memsz == 0 {
                continue;
            }

            let flags = PageFlags::from(kif::Perm::from(elf::PF::from_bits_truncate(phdr.flags)));
            let offset = math::round_dn(phdr.offset as usize, PAGE_SIZE);
            let virt = math::round_dn(phdr.vaddr, PAGE_SIZE);

            // do we need new memory for this segment?
            if (copy && flags.contains(PageFlags::W)) || phdr.filesz == 0 {
                let size =
                    math::round_up((phdr.vaddr & PAGE_MASK) + phdr.memsz as usize, PAGE_SIZE);

                if vpe.pe_desc().has_virtmem() {
                    let mut phys = mem::get().allocate(size as goff, PAGE_SIZE as goff)?;
                    Self::load_segment(vpe, phys.global(), virt as goff, size, flags, true)?;
                    phys.claim();
                }

                if phdr.filesz == 0 {
                    ktcu::clear(vpe.pe_id(), virt as goff, size)?;
                }
                else {
                    ktcu::copy(
                        // destination
                        vpe.pe_id(),
                        virt as goff,
                        // source
                        mod_addr.pe(),
                        mod_addr.offset() + offset as goff,
                        size,
                    )?;
                }

                end = virt + size;
            }
            else {
                assert!(phdr.memsz == phdr.filesz);
                let size = (phdr.offset as usize & PAGE_MASK) + phdr.filesz as usize;
                Self::load_segment(
                    vpe,
                    mod_addr + offset as goff,
                    virt as goff,
                    size,
                    flags,
                    true,
                )?;
                end = virt + size;
            }
        }

        // create initial heap
        let end = math::round_up(end, LPAGE_SIZE);
        let mut phys = mem::get().allocate(MOD_HEAP_SIZE as goff, PAGE_SIZE as goff)?;
        Self::load_segment(
            vpe,
            phys.global(),
            end as goff,
            MOD_HEAP_SIZE,
            PageFlags::RW,
            true,
        )?;
        phys.claim();

        Ok(hdr.entry)
    }

    fn write_arguments(addr: goff, pe: tcu::PEId, args: &[&str]) -> usize {
        let mut argptr: Vec<u64> = Vec::new();
        let mut argbuf: Vec<u8> = Vec::new();

        let off = addr + util::size_of::<envdata::EnvData>() as goff;
        let mut argoff = ENV_START + util::size_of::<envdata::EnvData>();
        for s in args {
            // push argv entry
            argptr.push(argoff as u64);

            // push string
            let arg = s.as_bytes();
            argbuf.extend_from_slice(arg);

            // 0-terminate it
            argbuf.push('\0' as u8);

            argoff += arg.len() + 1;
        }

        ktcu::write_mem(pe, off as goff, argbuf.as_ptr() as *const u8, argbuf.len());
        let argv_size = argptr.len() * util::size_of::<u64>();
        argoff = math::round_up(argoff, util::size_of::<u64>());
        ktcu::write_mem(
            pe,
            addr + (argoff - ENV_START) as goff,
            argptr.as_ptr() as *const u8,
            argv_size,
        );
        argoff
    }
}
