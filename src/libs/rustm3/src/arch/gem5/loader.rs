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

use cfg;
use col::Vec;
use com::MemGate;
use core::iter;
use elf;
use errors::{Code, Error};
use goff;
use io::read_object;
use kif;
use mem::heap;
use session::Pager;
use util;
use vfs::{BufReader, FileRef, Seek, SeekMode};
use vpe::Mapper;

pub struct Loader<'l> {
    pager: Option<&'l Pager>,
    pager_inherited: bool,
    mapper: &'l mut dyn Mapper,
    mem: &'l MemGate,
}

impl<'l> Loader<'l> {
    pub fn new(
        pager: Option<&'l Pager>,
        pager_inherited: bool,
        mapper: &'l mut dyn Mapper,
        mem: &'l MemGate,
    ) -> Loader<'l> {
        Loader {
            pager,
            pager_inherited,
            mapper,
            mem,
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

        let addr = |sym: &u8| (sym as *const u8) as usize;

        // use COW if both have a pager
        if let Some(pg) = self.pager {
            if self.pager_inherited {
                return pg.clone().map(|_| unsafe { addr(&_start) });
            }
            // TODO handle that case
            unimplemented!();
        }

        unsafe {
            // copy text
            let text_start = addr(&_text_start);
            let text_end = addr(&_text_end);
            self.mem
                .write_bytes(&_text_start, text_end - text_start, text_start as goff)?;

            // copy data and heap
            let data_start = addr(&_data_start);
            self.mem.write_bytes(
                &_data_start,
                heap::used_end() - data_start,
                data_start as goff,
            )?;

            // copy end-area of heap
            let heap_area_size = util::size_of::<heap::HeapArea>();
            self.mem.write_bytes(
                heap::end() as *const u8,
                heap_area_size,
                heap::end() as goff,
            )?;

            // copy stack
            self.mem
                .write_bytes(sp as *const u8, cfg::STACK_TOP - sp, sp as goff)?;

            Ok(addr(&_start))
        }
    }

    fn load_segment(
        &mut self,
        file: &mut BufReader<FileRef>,
        phdr: &elf::Phdr,
        buf: &mut [u8],
    ) -> Result<(), Error> {
        let prot = kif::Perm::from(elf::PF::from_bits_truncate(phdr.flags));
        let size = util::round_up(phdr.memsz as usize, cfg::PAGE_SIZE);

        let needs_init = if phdr.memsz == phdr.filesz {
            self.mapper.map_file(
                self.pager,
                file,
                phdr.offset as usize,
                phdr.vaddr as goff,
                size,
                prot,
            )
        }
        else {
            assert!(phdr.filesz == 0);
            self.mapper
                .map_anon(self.pager, phdr.vaddr as goff, size, prot)
        }?;

        if needs_init {
            self.mapper.init_mem(
                buf,
                &self.mem,
                file,
                phdr.offset as usize,
                phdr.filesz as usize,
                phdr.vaddr as goff,
                phdr.memsz as usize,
            )
        }
        else {
            Ok(())
        }
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

        // create area for boot/runtime stuff
        self.mapper.map_anon(
            self.pager,
            cfg::ENV_START as goff,
            cfg::ENV_SIZE,
            kif::Perm::RW,
        )?;

        // create area for stack
        self.mapper.map_anon(
            self.pager,
            cfg::STACK_BOTTOM as goff,
            cfg::STACK_SIZE,
            kif::Perm::RW,
        )?;

        // create heap
        // TODO align heap to 2M to use huge pages
        let heap_begin = util::round_up(end, cfg::PAGE_SIZE);
        let heap_size = if self.pager.is_some() {
            cfg::APP_HEAP_SIZE
        }
        else {
            cfg::MOD_HEAP_SIZE
        };
        self.mapper
            .map_anon(self.pager, heap_begin as goff, heap_size, kif::Perm::RW)?;

        Ok(hdr.entry)
    }

    pub fn write_arguments<I, S>(&self, off: &mut usize, args: I) -> Result<usize, Error>
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

        self.mem.write(&argbuf, *off as goff)?;
        argoff = util::round_up(argoff, util::size_of::<u64>());
        self.mem.write(&argptr, argoff as goff)?;

        *off = argoff + argptr.len() * util::size_of::<u64>();
        Ok(argoff as usize)
    }
}
