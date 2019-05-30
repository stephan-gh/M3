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

#include <base/Init.h>

#include "mem/MainMemory.h"
#include "mem/MemoryModule.h"
#include "pes/VPE.h"
#include "Args.h"
#include "DTU.h"
#include "Platform.h"

namespace kernel {

m3::PEDesc *Platform::_pes;
m3::BootInfo::Mod *Platform::_mods;
INIT_PRIO_USER(2) m3::BootInfo Platform::_info;
INIT_PRIO_USER(3) Platform::Init Platform::_init;

// note that we currently assume here, that compute PEs and memory PEs are not mixed
static peid_t last_pe_id;

Platform::Init::Init() {
    m3::BootInfo *info = &Platform::_info;
    // read kernel env
    peid_t pe = m3::DTU::gaddr_to_pe(m3::env()->kenv);
    goff_t addr = m3::DTU::gaddr_to_virt(m3::env()->kenv);
    DTU::get().read_mem(VPEDesc(pe, VPE::INVALID_ID), addr, info, sizeof(*info));
    addr += sizeof(*info);

    // read boot modules
    size_t total_mod_size = info->mod_size + sizeof(m3::BootInfo::Mod);
    Platform::_mods = reinterpret_cast<m3::BootInfo::Mod*>(malloc(total_mod_size));
    DTU::get().read_mem(VPEDesc(pe, VPE::INVALID_ID), addr, Platform::_mods, info->mod_size);
    addr += info->mod_size;

    // read PE descriptions
    size_t pe_size = sizeof(m3::PEDesc) * info->pe_count;
    Platform::_pes = new m3::PEDesc[info->pe_count];
    DTU::get().read_mem(VPEDesc(pe, VPE::INVALID_ID), addr, Platform::_pes, pe_size);

    // register memory modules
    int count = 0;
    size_t memidx = 0;
    const goff_t USABLE_MEM = (static_cast<goff_t>(2048) + 512) * 1024 * 1024;
    MainMemory &mem = MainMemory::get();
    for(size_t i = 0; i < info->pe_count; ++i) {
        m3::PEDesc pedesc = Platform::_pes[i];
        if(pedesc.type() == m3::PEType::MEM) {
            if(memidx >= ARRAY_SIZE(info->mems))
                PANIC("Not enough memory slots in boot info");

            // the first memory module hosts the FS image and other stuff
            if(count == 0) {
                if(pedesc.mem_size() <= USABLE_MEM + Args::kmem)
                    PANIC("Not enough DRAM");
                mem.add(new MemoryModule(MemoryModule::OCCUPIED, i, 0, USABLE_MEM));
                info->mems[memidx++] = m3::BootInfo::Mem(USABLE_MEM, true);

                mem.add(new MemoryModule(MemoryModule::KERNEL, i, USABLE_MEM, Args::kmem));

                size_t usize = pedesc.mem_size() - (USABLE_MEM + Args::kmem);
                mem.add(new MemoryModule(MemoryModule::USER, i, USABLE_MEM + Args::kmem, usize));
                info->mems[memidx++] = m3::BootInfo::Mem(usize, false);
            }
            else {
                mem.add(new MemoryModule(MemoryModule::USER, i, 0, pedesc.mem_size()));
                info->mems[memidx++] = m3::BootInfo::Mem(pedesc.mem_size(), false);
            }
            count++;
        }
        else
            last_pe_id = i;
    }

    // write-back boot info (changes to mems)
    addr = m3::DTU::gaddr_to_virt(m3::env()->kenv);
    DTU::get().write_mem(VPEDesc(pe, VPE::INVALID_ID), addr, info, sizeof(*info));
}

void Platform::add_modules(int, char **) {
    // unused
}

gaddr_t Platform::info_addr() {
    return m3::env()->kenv;
}

peid_t Platform::kernel_pe() {
    // gem5 initializes the peid for us
    return m3::env()->pe;
}
peid_t Platform::first_pe() {
    return m3::env()->pe + 1;
}
peid_t Platform::last_pe() {
    return last_pe_id;
}

goff_t Platform::def_recvbuf(peid_t no) {
    if(pe(no).has_virtmem())
        return RECVBUF_SPACE;
    return pe(no).mem_size() - RECVBUF_SIZE_SPM;
}

}
