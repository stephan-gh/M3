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

use core::{cmp, iter};

use crate::cfg;
use crate::col::Vec;
use crate::com::MemGate;
use crate::elf;
use crate::errors::{Code, Error};
use crate::goff;
use crate::io::{read_object, Read};
use crate::kif;
use crate::math;
use crate::mem::heap;
use crate::pes::{Mapper, VPE};
use crate::session::{MapFlags, Pager};
use crate::tcuif;
use crate::util;
use crate::vec;
use crate::vfs::{BufReader, FileRef, Seek, SeekMode};

extern "C" {
    static _start: u8;
    static _text_start: u8;
    static _text_end: u8;
    static _data_start: u8;
    static _bss_end: u8;
}

fn sym_addr<T>(sym: &T) -> usize {
    sym as *const _ as usize
}

pub fn copy_vpe(sp: usize, mem: MemGate) -> Result<usize, Error> {
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

pub fn clone_vpe(pager: &Pager) -> Result<usize, Error> {
    if VPE::cur().pager().is_some() {
        let entry = pager.clone().map(|_| unsafe { sym_addr(&_start) })?;
        // after cloning the address space we have to make sure that we don't have dirty cache lines
        // anymore. otherwise, if our child takes over a frame from us later and we writeback such
        // a cacheline afterwards, things break.
        tcuif::TCUIf::flush_invalidate()?;
        return Ok(entry);
    }

    // TODO handle that case
    unimplemented!();
}

pub fn load_program(
    vpe: &VPE,
    mapper: &mut dyn Mapper,
    file: &mut BufReader<FileRef>,
) -> Result<usize, Error> {
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

        load_segment(vpe, mapper, file, &phdr, &mut *buf)?;

        end = phdr.vaddr + phdr.memsz as usize;
    }

    // create area for stack
    mapper.map_anon(
        vpe.pager(),
        cfg::STACK_BOTTOM as goff,
        cfg::STACK_SIZE,
        kif::Perm::RW,
        MapFlags::PRIVATE | MapFlags::UNINIT,
    )?;

    // create heap
    let heap_begin = math::round_up(end, cfg::LPAGE_SIZE);
    let (heap_size, flags) = if vpe.pager().is_some() {
        (cfg::APP_HEAP_SIZE, MapFlags::NOLPAGE)
    }
    else {
        (cfg::MOD_HEAP_SIZE, MapFlags::empty())
    };
    mapper.map_anon(
        vpe.pager(),
        heap_begin as goff,
        heap_size,
        kif::Perm::RW,
        MapFlags::PRIVATE | MapFlags::UNINIT | flags,
    )?;

    Ok(hdr.entry)
}

pub fn write_arguments<I, S>(mem: &MemGate, off: &mut usize, args: I) -> Result<usize, Error>
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

fn load_segment(
    vpe: &VPE,
    mapper: &mut dyn Mapper,
    file: &mut BufReader<FileRef>,
    phdr: &elf::Phdr,
    buf: &mut [u8],
) -> Result<(), Error> {
    let prot = kif::Perm::from(elf::PF::from_bits_truncate(phdr.flags));
    let size = math::round_up(phdr.memsz as usize, cfg::PAGE_SIZE);

    let needs_init = if phdr.memsz == phdr.filesz {
        mapper.map_file(
            vpe.pager(),
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
        mapper.map_anon(
            vpe.pager(),
            phdr.vaddr as goff,
            size,
            prot,
            MapFlags::PRIVATE,
        )
    }?;

    if needs_init {
        let mem = vpe.get_mem(
            phdr.vaddr as goff,
            math::round_up(size, cfg::PAGE_SIZE) as goff,
            kif::Perm::W,
        )?;
        init_mem(
            buf,
            &mem,
            file,
            phdr.offset as usize,
            phdr.filesz as usize,
            phdr.memsz as usize,
        )
    }
    else {
        Ok(())
    }
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

    clear_mem(buf, mem, segoff, (memsize - fsize) as usize)
}

fn clear_mem(buf: &mut [u8], mem: &MemGate, mut virt: usize, mut len: usize) -> Result<(), Error> {
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
