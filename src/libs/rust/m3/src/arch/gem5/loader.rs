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

use cap::Selector;
use cfg;
use col::Vec;
use com::MemGate;
use core::{cmp, iter};
use elf;
use errors::{Code, Error};
use goff;
use io::{read_object, Read};
use kif;
use math;
use mem::heap;
use pes::Mapper;
use session::{MapFlags, Pager};
use syscalls;
use util;
use vfs::{BufReader, FileRef, Seek, SeekMode};

pub struct Loader<'l> {
    vpe: Selector,
    mem_sel: Selector,
    pe_desc: kif::PEDesc,
    pager: Option<&'l Pager>,
    pager_inherited: bool,
    mapper: &'l mut dyn Mapper,
}

fn sym_addr(sym: &u8) -> usize {
    sym as *const u8 as usize
}

impl<'l> Loader<'l> {
    pub fn new(
        vpe: Selector,
        mem_sel: Selector,
        pe_desc: kif::PEDesc,
        pager: Option<&'l Pager>,
        pager_inherited: bool,
        mapper: &'l mut dyn Mapper,
    ) -> Loader<'l> {
        Loader {
            vpe,
            mem_sel,
            pe_desc,
            pager,
            pager_inherited,
            mapper,
        }
    }

    pub fn copy_regions(&mut self, sp: usize) -> Result<usize, Error> {
        extern "C" {
            static _start: u8;
            static _text_start: u8;
            static _text_end: u8;
            static _data_start: u8;
            static _bss_end: u8;
        }

        if let Some(pg) = self.pager {
            if self.pager_inherited {
                return pg.clone().map(|_| unsafe { sym_addr(&_start) });
            }
            // TODO handle that case
            unimplemented!();
        }

        let mem = self.get_mem(0, self.pe_desc.mem_size())?;

        unsafe {
            // copy text
            let text_start = sym_addr(&_text_start);
            let text_end = sym_addr(&_text_end);
            mem.write_bytes(&_text_start, text_end - text_start, text_start as goff)?;

            // copy data and heap
            let data_start = sym_addr(&_data_start);
            mem.write_bytes(
                &_data_start,
                heap::used_end() - data_start,
                data_start as goff,
            )?;

            // copy end-area of heap
            let heap_area_size = util::size_of::<heap::HeapArea>();
            mem.write_bytes(
                heap::end() as *const u8,
                heap_area_size,
                heap::end() as goff,
            )?;

            // copy stack
            mem.write_bytes(sp as *const u8, cfg::STACK_TOP - sp, sp as goff)?;

            Ok(sym_addr(&_start))
        }
    }

    fn load_segment(
        &mut self,
        file: &mut BufReader<FileRef>,
        phdr: &elf::Phdr,
        buf: &mut [u8],
    ) -> Result<(), Error> {
        let prot = kif::Perm::from(elf::PF::from_bits_truncate(phdr.flags));
        let size = math::round_up(phdr.memsz as usize, cfg::PAGE_SIZE);

        let needs_init = if phdr.memsz == phdr.filesz {
            self.mapper.map_file(
                self.pager,
                file,
                phdr.offset as usize,
                phdr.vaddr as goff,
                size,
                prot,
                MapFlags::PRIVATE,
            )
        }
        else {
            assert!(phdr.filesz == 0);
            self.mapper.map_anon(
                self.pager,
                phdr.vaddr as goff,
                size,
                prot,
                MapFlags::PRIVATE,
            )
        }?;

        if needs_init {
            let mem = self.get_mem(phdr.vaddr as goff, math::round_up(size, cfg::PAGE_SIZE))?;
            let res = Self::init_mem(
                buf,
                &mem,
                file,
                phdr.offset as usize,
                phdr.filesz as usize,
                phdr.memsz as usize,
            );
            let crd = kif::CapRngDesc::new(kif::CapType::OBJECT, self.mem_sel, 1);
            syscalls::revoke(kif::SEL_VPE, crd, true).ok();
            res
        }
        else {
            Ok(())
        }
    }

    fn get_mem(&self, addr: goff, size: usize) -> Result<MemGate, Error> {
        syscalls::create_mgate(self.mem_sel, self.vpe, addr, size, kif::Perm::W)?;
        Ok(MemGate::new_owned_bind(self.mem_sel))
    }

    pub fn load_program(&mut self, file: &mut BufReader<FileRef>) -> Result<usize, Error> {
        let mut buf = vec![0u8; 4096];
        let hdr: elf::Ehdr = read_object(file)?;

        if hdr.ident[0] != b'\x7F'
            || hdr.ident[1] != b'E'
            || hdr.ident[2] != b'L'
            || hdr.ident[3] != b'F'
        {
            return Err(Error::new(Code::InvalidElf));
        }

        // copy load segments to destination PE
        let mut end = 0;
        let mut off = hdr.phoff;
        for _ in 0..hdr.phnum {
            // load program header
            file.seek(off, SeekMode::SET)?;
            let phdr: elf::Phdr = read_object(file)?;
            off += hdr.phentsize as usize;

            // we're only interested in non-empty load segments
            if phdr.ty != elf::PT::LOAD.val || phdr.memsz == 0 {
                continue;
            }

            self.load_segment(file, &phdr, &mut *buf)?;

            end = phdr.vaddr + phdr.memsz as usize;
        }

        // create area for stack
        self.mapper.map_anon(
            self.pager,
            cfg::STACK_BOTTOM as goff,
            cfg::STACK_SIZE,
            kif::Perm::RW,
            MapFlags::PRIVATE | MapFlags::UNINIT,
        )?;

        // create heap
        let heap_begin = math::round_up(end, cfg::LPAGE_SIZE);
        let (heap_size, flags) = if self.pager.is_some() {
            (cfg::APP_HEAP_SIZE, MapFlags::NOLPAGE)
        }
        else {
            (cfg::MOD_HEAP_SIZE, MapFlags::empty())
        };
        self.mapper.map_anon(
            self.pager,
            heap_begin as goff,
            heap_size,
            kif::Perm::RW,
            MapFlags::PRIVATE | MapFlags::UNINIT | flags,
        )?;

        Ok(hdr.entry)
    }

    fn init_mem(
        buf: &mut [u8],
        mem: &MemGate,
        file: &mut BufReader<FileRef>,
        foff: usize,
        fsize: usize,
        memsize: usize,
    ) -> Result<(), Error> {
        file.seek(foff, SeekMode::SET)?;

        let mut count = fsize;
        let mut segoff = 0;
        while count > 0 {
            let amount = cmp::min(count, buf.len());
            let amount = file.read(&mut buf[0..amount])?;

            mem.write(&buf[0..amount], segoff as goff)?;

            count -= amount;
            segoff += amount;
        }

        Self::clear_mem(buf, mem, segoff, (memsize - fsize) as usize)
    }

    fn clear_mem(
        buf: &mut [u8],
        mem: &MemGate,
        mut virt: usize,
        mut len: usize,
    ) -> Result<(), Error> {
        if len == 0 {
            return Ok(());
        }

        for it in buf.iter_mut() {
            *it = 0;
        }

        while len > 0 {
            let amount = cmp::min(len, buf.len());
            mem.write(&buf[0..amount], virt as goff)?;
            len -= amount;
            virt += amount;
        }

        Ok(())
    }

    pub fn write_arguments<I, S>(
        &mut self,
        mem: &MemGate,
        off: &mut usize,
        args: I,
    ) -> Result<usize, Error>
    where
        I: iter::IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut argptr = Vec::<u64>::new();
        let mut argbuf = Vec::new();

        let mut argoff = *off;
        for s in args {
            // push argv entry
            argptr.push(argoff as u64);

            // push string
            let arg = s.as_ref().as_bytes();
            argbuf.extend_from_slice(arg);

            // 0-terminate it
            argbuf.push(b'\0');

            argoff += arg.len() + 1;
        }

        mem.write_bytes(
            argbuf.as_ptr() as *const _,
            argbuf.len(),
            (*off - cfg::ENV_START) as goff,
        )?;

        argoff = math::round_up(argoff, util::size_of::<u64>());
        mem.write_bytes(
            argptr.as_ptr() as *const _,
            argptr.len() * util::size_of::<u64>(),
            (argoff - cfg::ENV_START) as goff,
        )?;

        *off = argoff + argptr.len() * util::size_of::<u64>();
        Ok(argoff as usize)
    }
}
