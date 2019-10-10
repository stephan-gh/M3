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

#include <base/util/Math.h>
#include <base/log/Kernel.h>
#include <base/ELF.h>

#include "mem/MainMemory.h"
#include "pes/VPE.h"
#include "DTU.h"
#include "Platform.h"

namespace kernel {

static uint64_t loaded = 0;

static const m3::BootInfo::Mod *get_mod(const char *name, bool *first) {
    size_t i = 0;
    size_t namelen = strlen(name);
    for(auto mod = Platform::mods_begin(); mod != Platform::mods_end(); ++mod, ++i) {
        if(strncmp(mod->name, name, namelen) == 0 &&
           (mod->name[namelen] == '\0' || mod->name[namelen] == ' ')) {
            *first = (loaded & (static_cast<uint64_t>(1) << i)) == 0;
            loaded |= static_cast<uint64_t>(1) << i;
            return &*mod;
        }
    }
    return nullptr;
}

static gaddr_t alloc_mem(size_t size) {
    MainMemory::Allocation alloc = MainMemory::get().allocate(size, PAGE_SIZE);
    if(!alloc)
        PANIC("Not enough memory");
    return m3::DTU::build_gaddr(alloc.pe(), alloc.addr);
}

static void read_from_mod(const m3::BootInfo::Mod *mod, void *data, size_t size, size_t offset) {
    if(offset + size < offset || offset + size > mod->size)
        PANIC("Invalid ELF file: offset invalid");

    goff_t addr = m3::DTU::gaddr_to_virt(mod->addr + offset);
    peid_t pe = m3::DTU::gaddr_to_pe(mod->addr + offset);
    DTU::get().read_mem(VPEDesc(pe, VPE::INVALID_ID), addr, data, size);
}

static void copy_clear(const VPEDesc &vpe, uintptr_t virt, gaddr_t phys, size_t size, bool clear) {
    DTU::get().copy_clear(vpe, virt,
        VPEDesc(m3::DTU::gaddr_to_pe(phys), VPE::INVALID_ID),
        m3::DTU::gaddr_to_virt(phys),
        size, clear);
}

static void map_segment(VPE &vpe, gaddr_t phys, goff_t virt, size_t size, int perms) {
    if(Platform::pe(vpe.pe()).has_virtmem() || (perms & MapCapability::EXCL)) {
        capsel_t dst = virt >> PAGE_BITS;
        size_t pages = m3::Math::round_up(size, PAGE_SIZE) >> PAGE_BITS;
        vpe.kmem()->alloc(vpe, sizeof(MapObject) + sizeof(MapCapability));
        if(perms & MapCapability::EXCL)
            vpe.kmem()->alloc(vpe, pages * PAGE_SIZE);
        // these mappings cannot be changed or revoked by applications
        perms |= MapCapability::KERNEL;
        auto mapcap = new MapCapability(&vpe.mapcaps(), dst, pages, new MapObject(phys, perms));
        vpe.mapcaps().set(dst, mapcap);
    }

    if(!Platform::pe(vpe.pe()).has_virtmem())
        copy_clear(vpe.desc(), static_cast<uintptr_t>(virt), phys, size, false);
}

static goff_t load_mod(VPE &vpe, const m3::BootInfo::Mod *mod, bool copy, bool is_idle, bool to_mem) {
    // load and check ELF header
    m3::ElfEh header;
    read_from_mod(mod, &header, sizeof(header), 0);

    if(header.e_ident[0] != '\x7F' || header.e_ident[1] != 'E' || header.e_ident[2] != 'L' ||
        header.e_ident[3] != 'F')
        PANIC("Invalid ELF file: invalid magic number");

    // map load segments
    goff_t end = 0;
    size_t off = header.e_phoff;
    for(uint i = 0; i < header.e_phnum; ++i, off += header.e_phentsize) {
        /* load program header */
        m3::ElfPh pheader;
        read_from_mod(mod, &pheader, sizeof(pheader), off);

        // we're only interested in non-empty load segments
        if(pheader.p_type != m3::PT_LOAD || pheader.p_memsz == 0)
            continue;

        int perms = 0;
        if(pheader.p_flags & m3::PF_R)
            perms |= m3::DTU::PTE_R;
        if(pheader.p_flags & m3::PF_W)
            perms |= m3::DTU::PTE_W;
        if(pheader.p_flags & m3::PF_X)
            perms |= m3::DTU::PTE_X;

        goff_t offset = m3::Math::round_dn(static_cast<size_t>(pheader.p_offset), PAGE_SIZE);
        goff_t virt = m3::Math::round_dn(static_cast<size_t>(pheader.p_vaddr), PAGE_SIZE);

        if(is_idle) {
            gaddr_t phys_base = Platform::pe_mem_base() + vpe.pe() * Platform::pe_mem_size();
            gaddr_t phys = phys_base + virt;

            size_t size = (pheader.p_offset & PAGE_BITS) + pheader.p_memsz;
            map_segment(vpe, phys, virt, size, perms);
            end = virt + size;

            // workaround for ARM: if we push remotely into the cache, it gets loaded to the L1d
            // cache. however, we push instructions which need to end up in L1i. Thus, write to mem.
            if(virt == 0x0 && Platform::pe(vpe.pe()).has_virtmem()) {
                VPEDesc tgt(m3::DTU::gaddr_to_pe(phys), VPE::INVALID_ID);
                copy_clear(tgt, m3::DTU::gaddr_to_virt(phys),
                           mod->addr + offset, size, pheader.p_filesz == 0);
            }
        }
        // do we need new memory for this segment?
        else if((copy && (perms & m3::DTU::PTE_W)) || pheader.p_filesz == 0) {
            // allocate memory
            size_t size = static_cast<size_t>((pheader.p_vaddr & PAGE_BITS) + pheader.p_memsz);
            size = m3::Math::round_up(size, PAGE_SIZE);
            gaddr_t phys = alloc_mem(size);

            // map it
            map_segment(vpe, phys, virt, size, perms | MapCapability::EXCL);
            end = virt + size;

            // initialize it
            VPEDesc tgt = to_mem ? VPEDesc(m3::DTU::gaddr_to_pe(phys), VPE::INVALID_ID) : vpe.desc();
            copy_clear(tgt, virt, mod->addr + offset, size, pheader.p_filesz == 0);
        }
        else {
            assert(pheader.p_memsz == pheader.p_filesz);
            size_t size = (pheader.p_offset & PAGE_BITS) + pheader.p_filesz;
            map_segment(vpe, mod->addr + offset, virt, size, perms);
            end = virt + size;
        }
    }

    if(!is_idle) {
        // create initial heap
        gaddr_t phys = alloc_mem(ROOT_HEAP_SIZE);
        goff_t virt = m3::Math::round_up(end, static_cast<goff_t>(PAGE_SIZE));
        map_segment(vpe, phys, virt, ROOT_HEAP_SIZE, m3::DTU::PTE_RW | MapCapability::EXCL);
    }

    return header.e_entry;
}

static goff_t map_idle(VPE &vpe) {
    bool first;
    const m3::BootInfo::Mod *idle = get_mod("pemux", &first);
    if(!idle)
        PANIC("Unable to find boot module 'pemux'");

    // load idle
    goff_t res = load_mod(vpe, idle, true, true, Platform::pe(vpe.pe()).has_mmu());

    // clear PEMUX_*
    if(Platform::pe(vpe.pe()).has_mmu()) {
        gaddr_t phys = idle->addr + PEMUX_YIELD;
        DTU::get().copy_clear(VPEDesc(m3::DTU::gaddr_to_pe(phys), VPE::INVALID_ID),
            m3::DTU::gaddr_to_virt(phys),
            VPEDesc(0, 0), 0, // unused
            16, true);

        // map DTU
        int perm = m3::DTU::PTE_RW | m3::DTU::PTE_I | m3::DTU::PTE_UNCACHED;
        map_segment(vpe, 0xF0000000, 0xF0000000, PAGE_SIZE, perm);
        // map the privileged registers only for ring 0
        map_segment(vpe, 0xF0001000, 0xF0001000, PAGE_SIZE, m3::DTU::PTE_RW | m3::DTU::PTE_UNCACHED);
    }
    return res;
}

void VPE::load_app() {
    assert(_flags & F_BOOTMOD);

    bool appFirst;
    const m3::BootInfo::Mod *mod = get_mod("root", &appFirst);
    if(!mod)
        PANIC("Unable to find boot module 'root'");

    if(Platform::pe(pe()).has_virtmem()) {
        // map runtime space
        goff_t virt = ENV_START;
        gaddr_t phys = alloc_mem(STACK_TOP - virt);
        map_segment(*this, phys, virt, STACK_TOP - virt, m3::DTU::PTE_RW | MapCapability::EXCL);
    }

    // load app
    goff_t entry = load_mod(*this, mod, !appFirst, false, false);

    // copy arguments and arg pointers to buffer
    static const char *uargv[] = {"root"};
    char buffer[64];
    uint64_t *argptr = reinterpret_cast<uint64_t*>(buffer);
    char *args = buffer + 1 * sizeof(uint64_t);
    size_t off = static_cast<size_t>(args - buffer);
    *argptr++ = ENV_SPACE_START + off;
    strcpy(args, uargv[0]);

    // write buffer to the target PE
    size_t argssize = off + sizeof("root");
    DTU::get().write_mem(desc(), ENV_SPACE_START, buffer, argssize);

    // write env to targetPE
    m3::Env senv;
    memset(&senv, 0, sizeof(senv));

    senv.argc = 1;
    senv.argv = ENV_SPACE_START;
    senv.sp = STACK_TOP - sizeof(word_t);
    senv.entry = entry;
    senv.shared = VPE::USE_PEMUX;
    senv.pedesc = Platform::pe(pe());
    senv.heapsize = ROOT_HEAP_SIZE;
    senv.rmng_sel = m3::KIF::INV_SEL;
    senv.kmem_sel = m3::KIF::FIRST_FREE_SEL;
    senv.caps = _first_sel;

    DTU::get().write_mem(desc(), ENV_START, &senv, sizeof(senv));
}

void VPE::init_memory() {
    bool vm = Platform::pe(pe()).has_virtmem();
    if(vm) {
        address_space()->setup(desc());
        // write all PTEs to memory until we have loaded PEMux
        if(Platform::pe(pe()).has_mmu())
            _state = VPE::DEAD;
    }

    // for SPM PEs, we don't need to do anything; PEMux has already been loaded
    if(vm) {
        if(Platform::pe(pe()).is_programmable())
            map_idle(*this);
        // PEs with virtual memory still need the PEMux flags
        else {
            gaddr_t phys = alloc_mem(PAGE_SIZE);
            map_segment(*this, phys, PEMUX_FLAGS & ~PAGE_MASK, PAGE_SIZE,
                        m3::DTU::PTE_RW | MapCapability::EXCL);
        }
    }

    // PEMux is ready; let it initialize itself
    DTU::get().wakeup(desc());
    // we can now write the PTEs to the VPE's address space
    if(Platform::pe(pe()).has_mmu())
        _state = VPE::RUNNING;

    if(vm) {
        // map receive buffer
        gaddr_t phys = alloc_mem(RECVBUF_SIZE);
        map_segment(*this, phys, RECVBUF_SPACE, RECVBUF_SIZE,
                    m3::DTU::PTE_RW | MapCapability::EXCL);
    }

    // boot modules are started implicitly
    if(_flags & F_BOOTMOD)
        load_app();
}

}
