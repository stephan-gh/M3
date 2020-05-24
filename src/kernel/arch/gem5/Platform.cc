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

#include "mem/MainMemory.h"
#include "mem/MemoryModule.h"
#include "pes/VPE.h"
#include "Args.h"
#include "TCU.h"
#include "Platform.h"

#include <memory>

namespace kernel {

m3::BootInfo::Mod *Platform::_mods;
m3::PEDesc *Platform::_pes;
m3::BootInfo::Mem *Platform::_mems;
m3::BootInfo Platform::_info;

// note that we currently assume here, that compute PEs and memory PEs are not mixed
static peid_t last_pe_id;

void Platform::init() {
    m3::BootInfo *info = &Platform::_info;
    // read kernel env
    m3::GlobAddr kenv(m3::env()->kenv);
    TCU::read_mem(kenv.pe(), kenv.offset(), info, sizeof(*info));

    // read boot modules
    m3::GlobAddr kenvmods(kenv + sizeof(*info));
    size_t mod_size = info->mod_count * sizeof(m3::BootInfo::Mod);
    Platform::_mods = new m3::BootInfo::Mod[info->mod_count];
    TCU::read_mem(kenvmods.pe(), kenvmods.offset(), Platform::_mods, mod_size);

    // read PE descriptions
    m3::GlobAddr kenvpes(kenvmods + mod_size);
    size_t pe_size = sizeof(m3::PEDesc) * info->pe_count;
    Platform::_pes = new m3::PEDesc[info->pe_count];
    TCU::read_mem(kenvpes.pe(), kenvpes.offset(), Platform::_pes, pe_size);

    // read memory regions
    m3::GlobAddr kenvmems(kenvpes + pe_size);
    size_t mem_size = sizeof(m3::BootInfo::Mem) * info->mem_count;
    Platform::_mems = new m3::BootInfo::Mem[info->mem_count];
    TCU::read_mem(kenvmems.pe(), kenvmems.offset(), Platform::_mems, mem_size);

    // build new info for user PEs
    m3::BootInfo uinfo;
    memcpy(&uinfo, info, sizeof(uinfo));
    auto umems = std::make_unique<m3::BootInfo::Mem[]>(info->mem_count);
    auto upes = std::make_unique<m3::BootInfo::PE[]>(info->pe_count - 1);

    // register memory modules
    size_t umemidx = 0, kmemidx = 0;
    size_t peidx = 0;
    MainMemory &mem = MainMemory::get();
    for(size_t i = 0; i < info->pe_count; ++i) {
        m3::PEDesc pedesc = Platform::_pes[i];
        if(pedesc.type() == m3::PEType::MEM) {
            // the first memory module hosts the FS image and other stuff
            if(umemidx == 0) {
                size_t avail = _mems[kmemidx].size();
                if(avail <= Args::kmem)
                    PANIC("Not enough DRAM for kernel memory (" << Args::kmem << ")");
                size_t used = pedesc.mem_size() - avail;
                mem.add(new MemoryModule(MemoryModule::OCCUPIED, m3::GlobAddr(i, 0), used));
                umems[umemidx++] = m3::BootInfo::Mem(0, used, true);

                mem.add(new MemoryModule(MemoryModule::KERNEL, m3::GlobAddr(i, used), Args::kmem));

                mem.add(new MemoryModule(MemoryModule::USER, m3::GlobAddr(i, used + Args::kmem), avail));
                umems[umemidx++] = m3::BootInfo::Mem(used + Args::kmem, avail - Args::kmem, false);
            }
            else {
                if(umemidx >= info->mem_count)
                    PANIC("Not enough memory slots in boot info");

                mem.add(new MemoryModule(MemoryModule::USER, m3::GlobAddr(i, 0), pedesc.mem_size()));
                umems[umemidx++] = m3::BootInfo::Mem(0, pedesc.mem_size(), false);
            }
            kmemidx++;
        }
        else {
            if(kmemidx > 0)
                PANIC("All memory PEs have to be last");
            last_pe_id = i;

            // don't hand out the kernel PE
            if(i > 0) {
                assert(kernel_pe() == 0);
                upes[peidx].id = i;
                upes[peidx].desc = pedesc;
                peidx++;
            }
        }
    }

    // write-back boot info
    uinfo.pe_count = peidx;
    uinfo.mem_count = umemidx;
    TCU::write_mem(kenv.pe(), kenv.offset(), &uinfo, sizeof(uinfo));
    // write-back user PEs
    TCU::write_mem(kenv.pe(), kenvpes.offset(),
                   upes.get(), sizeof(m3::BootInfo::PE) * uinfo.pe_count);
    // write-back user memory regions
    TCU::write_mem(kenv.pe(), kenvpes.offset() + sizeof(m3::BootInfo::PE) * uinfo.pe_count,
                   umems.get(), sizeof(m3::BootInfo::Mem) * uinfo.mem_count);
}

void Platform::add_modules(int, char **) {
    // unused
}

m3::GlobAddr Platform::info_addr() {
    return m3::GlobAddr(m3::env()->kenv);
}

peid_t Platform::kernel_pe() {
    // gem5 initializes the peid for us
    return m3::env()->pe_id;
}
peid_t Platform::first_pe() {
    return m3::env()->pe_id + 1;
}
peid_t Platform::last_pe() {
    return last_pe_id;
}

bool Platform::is_shared(peid_t no) {
    return pe(no).is_programmable();
}

goff_t Platform::rbuf_pemux(peid_t no) {
    if(pe(no).has_virtmem())
        return PEMUX_RBUF_PHYS;
    else
        return PEMUX_RBUF_SPACE;
}

}
