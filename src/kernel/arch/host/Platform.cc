/*
 * Copyright (C) 2016-2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/Config.h>
#include <base/log/Kernel.h>
#include <base/Init.h>

#include <sys/mman.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <unistd.h>
#include <vector>

#include "mem/MainMemory.h"
#include "Args.h"
#include "TCU.h"
#include "Platform.h"

namespace kernel {

m3::BootInfo::Mod *Platform::_mods;
m3::PEDesc *Platform::_pes;
m3::BootInfo::Mem *Platform::_mems;
m3::BootInfo Platform::_info;

static MainMemory::Allocation binfomem;

void Platform::init() {
    size_t cores = PE_COUNT;
    const char *cores_str = getenv("M3_CORES");
    if(cores_str) {
        cores = strtoul(cores_str, NULL, 10);
        if(cores < 2 || cores > PE_COUNT)
            PANIC("Invalid PE count (min=2, max=" << PE_COUNT << ")");
    }

    // init PEs
    size_t total_pes = cores + 1;
    if(Args::bridge)
        total_pes += 2;
    if(Args::disk)
        total_pes++;
    _info.pe_count = total_pes;
    _pes = new m3::PEDesc[total_pes];
    size_t i = 0;
    for(; i < cores; ++i)
        _pes[i] = m3::PEDesc(m3::PEType::COMP_IMEM, m3::PEISA::X86, 1024 * 1024);

    // these are dummy PEs; they do not really exist, but serve the purpose to let root not
    // complain that the IDE/NIC PE isn't present.
    if(Args::bridge) {
        _pes[i++] = m3::PEDesc(m3::PEType::COMP_IMEM, m3::PEISA::NIC, 0);
        _pes[i++] = m3::PEDesc(m3::PEType::COMP_IMEM, m3::PEISA::NIC, 0);
    }
    if(Args::disk)
        _pes[i++] = m3::PEDesc(m3::PEType::COMP_IMEM, m3::PEISA::IDE_DEV, 0);

    _pes[i++] = m3::PEDesc(m3::PEType::MEM, m3::PEISA::NONE, TOTAL_MEM_SIZE);

    // create memory
    uintptr_t base = reinterpret_cast<uintptr_t>(
        mmap(0, TOTAL_MEM_SIZE, PROT_READ | PROT_WRITE, MAP_ANON | MAP_PRIVATE, -1, 0));

    if(TOTAL_MEM_SIZE <= FS_MAX_SIZE + Args::kmem)
        PANIC("Not enough DRAM");

    MainMemory &mem = MainMemory::get();
    mem.add(new MemoryModule(MemoryModule::OCCUPIED, m3::GlobAddr(0, base), FS_MAX_SIZE));
    mem.add(new MemoryModule(MemoryModule::KERNEL, m3::GlobAddr(0, base + FS_MAX_SIZE), Args::kmem));
    size_t usize = TOTAL_MEM_SIZE - (FS_MAX_SIZE + Args::kmem);
    mem.add(new MemoryModule(MemoryModule::USER, m3::GlobAddr(0, base + FS_MAX_SIZE + Args::kmem), usize));

    // set memories
    _info.mem_count = 2;
    _mems = new m3::BootInfo::Mem[2];
    _mems[0] = m3::BootInfo::Mem(0, FS_MAX_SIZE, true);
    _mems[1] = m3::BootInfo::Mem(FS_MAX_SIZE + Args::kmem, usize, false);
}

void Platform::add_modules(int argc, char **argv) {
    MainMemory &mem = MainMemory::get();

    _mods = new m3::BootInfo::Mod[argc];

    size_t bmodsize = 0;
    for(int i = 0; i < argc; ++i) {
        m3::OStringStream args;
        args << basename(argv[i]);

        strcpy(_mods[i].name, args.str());

        bmodsize += sizeof(m3::BootInfo::Mod) + args.length() + 1;

        // copy boot module into memory
        {
            int fd = open(argv[i], O_RDONLY);
            if(fd < 0)
                PANIC("Opening '" << argv[i] << "' for reading failed");
            struct stat info;
            if(fstat(fd, &info) == -1)
                PANIC("Stat for '" << argv[i] << "' failed");

            MainMemory::Allocation alloc = mem.allocate(static_cast<size_t>(info.st_size), 1);
            if(!alloc)
                PANIC("Not enough memory for boot module '" << argv[i] << "'");
            ssize_t res = read(fd, reinterpret_cast<void*>(alloc.addr().offset()), alloc.size);
            if(res == -1)
                PANIC("Reading from '" << argv[i] << "' failed");
            close(fd);

            _mods[i].addr = alloc.addr().offset();
            _mods[i].size = alloc.size;
        }
    }

    // set modules
    _info.mod_count = static_cast<uint64_t>(argc);

    // build kinfo page
    size_t bsize = sizeof(m3::BootInfo) + _info.mod_count * sizeof(m3::BootInfo::Mod)
                                        + _info.pe_count * sizeof(m3::BootInfo::PE)
                                        + _info.mem_count * sizeof(m3::BootInfo::Mem);
    binfomem = mem.allocate(bsize, 1);
    if(!binfomem)
        PANIC("Not enough memory for boot info");
    m3::BootInfo *binfo = reinterpret_cast<m3::BootInfo*>(binfomem.addr().offset());
    memcpy(binfo, &_info, sizeof(_info));
    binfo->pe_count -= 2;

    // add modules
    uintptr_t mod_addr = binfomem.addr().offset() + sizeof(_info);
    for(uint64_t i = 0; i < _info.mod_count; ++i) {
        memcpy(reinterpret_cast<void*>(mod_addr), _mods + i, sizeof(_mods[i]));
        mod_addr += sizeof(_mods[i]);
    }

    // add PEs
    for(uint64_t i = 1; i < _info.pe_count - 1; ++i) {
        m3::BootInfo::PE pe;
        pe.id = i;
        pe.desc = Platform::_pes[i];
        memcpy(reinterpret_cast<void*>(mod_addr), &pe, sizeof(pe));
        mod_addr += sizeof(pe);
    }

    // add memory regions
    for(uint64_t i = 0; i < _info.mem_count; ++i) {
        memcpy(reinterpret_cast<void*>(mod_addr), _mems + i, sizeof(_mems[i]));
        mod_addr += sizeof(_mems[i]);
    }
}

m3::GlobAddr Platform::info_addr() {
    return binfomem.addr();
}

peid_t Platform::kernel_pe() {
    return 0;
}
peid_t Platform::first_pe() {
    return 1;
}
peid_t Platform::last_pe() {
    return _info.pe_count - 2;
}

bool Platform::is_shared(peid_t) {
    return false;
}

goff_t Platform::rbuf_pemux(peid_t) {
    // unused
    return 0;
}

}
