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

#include <base/log/Kernel.h>
#include <base/util/Math.h>
#include <base/Init.h>

#include <isr/ISR.h>

#include "mem/AddrSpace.h"
#include "pes/PEManager.h"
#include "pes/VPEManager.h"
#include "pes/VPE.h"
#include "DTU.h"
#include "Platform.h"

// defined in paging library (Rust)
extern "C" kernel::AddrSpace::mmu_pte_t to_mmu_pte(m3::DTU::pte_t);
extern "C" m3::DTU::pte_t to_dtu_pte(kernel::AddrSpace::mmu_pte_t pte);
extern "C" goff_t get_pte_addr(goff_t virt, int level);
extern "C" m3::DTU::pte_t get_pte(uint64_t virt, uint64_t perm);

namespace kernel {

static char buffer[4096];

void AddrSpace::mmu_cmd_remote(const VPEDesc &vpe, m3::DTU::reg_t arg) {
    assert(arg != 0);
    DTU::get().ext_request(vpe, arg);

    // wait until the remote core sends us an ACK (writes 0 to EXT_REQ)
    m3::DTU::reg_t mstreq = 1;
    goff_t extarg_addr = m3::DTU::priv_reg_addr(m3::DTU::PrivRegs::EXT_REQ);
    while(mstreq != 0)
        DTU::get().read_mem(vpe, extarg_addr, &mstreq, sizeof(mstreq));
}

void AddrSpace::setup(const VPEDesc &vpe) {
    // insert recursive entry
    goff_t addr = m3::DTU::gaddr_to_virt(_root);
    m3::DTU::pte_t pte = to_mmu_pte(_root | m3::DTU::PTE_RWX);
    DTU::get().write_mem(VPEDesc(m3::DTU::gaddr_to_pe(_root), VPE::INVALID_ID),
        addr + m3::DTU::PTE_REC_IDX * sizeof(pte), &pte, sizeof(pte));

    // invalidate TLB, because we have changed the root PT
    DTU::get().invtlb_remote(vpe);
}

size_t AddrSpace::max_kmem_for(size_t bytes) const {
    size_t pts = 0;
    // the root PT does always exist
    for(int i = 1; i < m3::DTU::LEVEL_CNT - 1; ++i) {
        const size_t ptsize = (1UL << (m3::DTU::LEVEL_BITS * i)) * PAGE_SIZE;
        pts += 2 + bytes / ptsize;
    }
    return pts * PAGE_SIZE;
}

void AddrSpace::clear_pt(gaddr_t pt) {
    // clear the pagetable
    memset(buffer, 0, sizeof(buffer));
    peid_t pe = m3::DTU::gaddr_to_pe(pt);
    goff_t addr = m3::DTU::gaddr_to_virt(pt);
    for(size_t i = 0; i < PAGE_SIZE / sizeof(buffer); ++i) {
        DTU::get().write_mem(VPEDesc(pe, VPE::INVALID_ID),
            addr + i * sizeof(buffer), buffer, sizeof(buffer));
    }
}

bool AddrSpace::create_pt(const VPEDesc &vpe, VPE *vpeobj, goff_t &virt, goff_t pteAddr,
                          m3::DTU::pte_t pte, gaddr_t &phys, uint &pages, int perm, int level) {
    // use a large page, if possible
    if(level == 1 && m3::Math::is_aligned(virt, m3::DTU::LPAGE_SIZE) &&
                     m3::Math::is_aligned(phys, m3::DTU::LPAGE_SIZE) &&
                     pages * PAGE_SIZE >= m3::DTU::LPAGE_SIZE) {
        pte = to_mmu_pte(phys | static_cast<uint>(perm) | m3::DTU::PTE_I | m3::DTU::PTE_LARGE);
        KLOG(PTES, "VPE" << _vpeid << ": lvl " << level << " PTE for "
            << m3::fmt(virt, "p") << ": " << m3::fmt(pte, "#0x", 16));
        DTU::get().write_mem(vpe, pteAddr, &pte, sizeof(pte));
        phys += m3::DTU::LPAGE_SIZE;
        virt += m3::DTU::LPAGE_SIZE;
        pages -= m3::DTU::LPAGE_SIZE / PAGE_SIZE;
        return true;
    }

    // create the pagetable on demand
    if(pte == 0) {
        // if we don't have a pagetable for that yet, unmapping is a noop
        if(perm == 0)
            return true;

        if(vpeobj) {
            UNUSED bool res = vpeobj->kmem()->alloc(*vpeobj, PAGE_SIZE);
            assert(res);
        }

        MainMemory::Allocation alloc = MainMemory::get().allocate(PAGE_SIZE, PAGE_SIZE);
        assert(alloc);

        // clear PT
        pte = m3::DTU::build_gaddr(alloc.pe(), alloc.addr);
        clear_pt(pte);

        // insert PTE
        pte |= m3::DTU::PTE_IRWX;
        pte = to_mmu_pte(pte);
        const size_t ptsize = (1UL << (m3::DTU::LEVEL_BITS * level)) * PAGE_SIZE;
        KLOG(PTES, "VPE" << _vpeid << ": lvl " << level << " PTE for "
            << m3::fmt(virt & ~(ptsize - 1), "p") << ": " << m3::fmt(pte, "#0x", 16)
            << (Platform::pe(vpe.pe).type() == m3::PEType::MEM ? " (to mem)" : ""));
        DTU::get().write_mem(vpe, pteAddr, &pte, sizeof(pte));
    }
    return false;
}

bool AddrSpace::create_ptes(const VPEDesc &vpe, goff_t &virt, goff_t pteAddr, m3::DTU::pte_t pte,
                            gaddr_t &phys, uint &pages, int perm) {
    // note that we can assume here that map_pages is always called for the same set of
    // pages. i.e., it is not possible that we map page 1 and 2 and afterwards remap
    // only page 1. this is because we call map_pages with MapCapability, which can't
    // be resized. thus, we know that a downgrade for the first, is a downgrade for all
    // and that an existing mapping for the first is an existing mapping for all.

    m3::DTU::pte_t pteDTU = to_dtu_pte(pte);
    m3::DTU::pte_t npte = phys | static_cast<uint>(perm);
    if(npte == pteDTU)
        return true;

    bool downgrade = false;
    // do not invalidate pages if we are writing to a memory PE
    if((pteDTU & m3::DTU::PTE_RWX) && Platform::pe(vpe.pe).has_virtmem())
        downgrade = ((pteDTU & m3::DTU::PTE_RWX) & (~npte & m3::DTU::PTE_RWX)) != 0;

    goff_t endpte = m3::Math::min(pteAddr + pages * sizeof(npte),
        m3::Math::round_up(pteAddr + sizeof(npte), static_cast<goff_t>(PAGE_SIZE)));

    uint count = (endpte - pteAddr) / sizeof(npte);
    assert(count > 0);
    pages -= count;
    phys += count << PAGE_BITS;

    npte = to_mmu_pte(npte);
    while(pteAddr < endpte) {
        size_t i = 0;
        goff_t startAddr = pteAddr;
        m3::DTU::pte_t buf[16];
        for(; pteAddr < endpte && i < ARRAY_SIZE(buf); ++i) {
            KLOG(PTES, "VPE" << _vpeid << ": lvl 0 PTE for "
                << m3::fmt(virt, "p") << ": " << m3::fmt(npte, "#0x", 16)
                << (downgrade ? " (invalidating)" : "")
                << (Platform::pe(vpe.pe).type() == m3::PEType::MEM ? " (to mem)" : ""));

            buf[i] = npte;

            pteAddr += sizeof(npte);
            virt += PAGE_SIZE;
            npte += PAGE_SIZE;
        }

        DTU::get().write_mem(vpe, startAddr, buf, i * sizeof(buf[0]));

        if(downgrade) {
            for(goff_t vaddr = virt - i * PAGE_SIZE; vaddr < virt; vaddr += PAGE_SIZE) {
                mmu_cmd_remote(vpe, vaddr | m3::DTU::ExtReqOpCode::INV_PAGE);
                DTU::get().invlpg_remote(vpe, vaddr);
            }
        }
    }
    return false;
}

goff_t AddrSpace::get_pte_addr_mem(const VPEDesc &vpe, gaddr_t root, goff_t virt, int level) {
    goff_t pt = m3::DTU::gaddr_to_virt(root);
    for(int l = m3::DTU::LEVEL_CNT - 1; l >= 0; --l) {
        size_t idx = (virt >> (PAGE_BITS + m3::DTU::LEVEL_BITS * l)) & m3::DTU::LEVEL_MASK;
        pt += idx * m3::DTU::PTE_SIZE;

        if(level == l)
            return pt;

        m3::DTU::pte_t pte;
        DTU::get().read_mem(vpe, pt, &pte, sizeof(pte));
        pte = to_dtu_pte(pte);

        pt = m3::DTU::gaddr_to_virt(pte & ~PAGE_MASK);
    }

    UNREACHED;
}

void AddrSpace::map_pages(const VPEDesc &vpe, goff_t virt, gaddr_t phys, uint pages, int perm) {
    VPE *vpeobj = vpe.pe != Platform::kernel_pe() ? &VPEManager::get().vpe(vpe.id) : nullptr;
    bool running = !vpeobj || vpeobj->is_on_pe();
    // just ignore the request if the VPE has already been stopped (we've set the idle addrspace then)
    if(vpeobj && (!Platform::pe(vpeobj->peid()).has_virtmem() || vpeobj->is_stopped()))
        return;

    KLOG(MAPPINGS, "VPE" << _vpeid << ": mapping "
        << m3::fmt(virt, "p") << ".." << m3::fmt(virt + pages * PAGE_SIZE - 1, "p")
        << " to "
        << m3::fmt(phys, "#0x", 16) << ".." << m3::fmt(phys + pages * PAGE_SIZE - 1, "#0x", 16)
        << " with " << m3::fmt(perm, "#x"));

    VPEDesc rvpe(vpe);
    gaddr_t root = 0;
    if(!running) {
        // TODO we currently assume that all PTEs are in the same mem PE as the root PT
        peid_t pe = m3::DTU::gaddr_to_pe(_root);
        root = _root;
        rvpe = VPEDesc(pe, VPE::INVALID_ID);
    }

    while(pages > 0) {
        for(int level = m3::DTU::LEVEL_CNT - 1; level >= 0; --level) {
            goff_t pteAddr;
            if(!running)
                pteAddr = get_pte_addr_mem(rvpe, root, virt, level);
            else
                pteAddr = get_pte_addr(virt, level);

            m3::DTU::pte_t pte;
            DTU::get().read_mem(rvpe, pteAddr, &pte, sizeof(pte));
            pte = to_dtu_pte(pte);

            if(level > 0) {
                if(create_pt(rvpe, vpeobj, virt, pteAddr, pte, phys, pages, perm, level))
                    break;
            }
            else {
                if(create_ptes(rvpe, virt, pteAddr, pte, phys, pages, perm))
                    return;
            }
        }
    }
}

void AddrSpace::unmap_pages(const VPEDesc &vpe, goff_t virt, uint pages) {
    // don't do anything if the VPE is already dead
    if(vpe.pe != Platform::kernel_pe() && VPEManager::get().vpe(vpe.id).state() == VPE::DEAD)
        return;

    map_pages(vpe, virt, 0, pages, 0);
}

void AddrSpace::remove_pts_rec(VPE &vpe, gaddr_t pt, goff_t virt, int level) {
    static_assert(sizeof(buffer) >= PAGE_SIZE, "Buffer smaller than a page");

    // load entire page table
    peid_t pe = m3::DTU::gaddr_to_pe(pt);
    VPEDesc memvpe(pe, VPE::INVALID_ID);
    DTU::get().read_mem(memvpe, m3::DTU::gaddr_to_virt(pt), buffer, PAGE_SIZE);

    // free all PTEs, if they point to page tables
    size_t ptsize = (1UL << (m3::DTU::LEVEL_BITS * level)) * PAGE_SIZE;
    m3::DTU::pte_t *ptes = reinterpret_cast<m3::DTU::pte_t*>(buffer);
    for(size_t i = 0; i < 1 << m3::DTU::LEVEL_BITS; ++i) {
        if(ptes[i]) {
            gaddr_t gaddr = to_dtu_pte(ptes[i]) & ~static_cast<gaddr_t>(PAGE_MASK);
            // not for the recursive entry
            if(level > 1 && !(level == m3::DTU::LEVEL_CNT - 1 && i == m3::DTU::PTE_REC_IDX)) {
                remove_pts_rec(vpe, gaddr, virt, level - 1);

                // reload the rest of the buffer
                size_t off = i * sizeof(*ptes);
                DTU::get().read_mem(memvpe, m3::DTU::gaddr_to_virt(pt + off), buffer + off, PAGE_SIZE - off);
            }

            // free kmem
            vpe.kmem()->free(vpe, PAGE_SIZE);

            // free page table
            KLOG(PTES, "VPE" << vpe.id() << ": lvl " << level << " PTE for " << m3::fmt(virt, "p") << " removed");
            MainMemory::get().free(MainMemory::get().build_allocation(gaddr, PAGE_SIZE));
        }

        virt += ptsize;
    }
}

void AddrSpace::remove_pts(vpeid_t vpe) {
    VPE &v = VPEManager::get().vpe(vpe);
    assert(v.state() == VPE::DEAD);

    // don't destroy page tables of idle VPEs. we need them to execute something on the other PEs
    remove_pts_rec(v, _root, 0, m3::DTU::LEVEL_CNT - 1);
}

#if defined(__x86_64__)
void AddrSpace::handle_xlate(m3::DTU::reg_t xlate_req) {
    m3::DTU &dtu = m3::DTU::get();

    uintptr_t virt = xlate_req & ~PAGE_MASK;
    uint perm = (xlate_req >> 1) & 0xF;
    uint xferbuf = (xlate_req >> 5) & 0x7;

    m3::DTU::pte_t pte = get_pte(virt, perm);
    if(~(pte & 0xF) & perm)
        PANIC("Pagefault during PT walk for " << virt << " (PTE=" << m3::fmt(pte, "p") << ")");

    // tell DTU the result
    dtu.set_core_resp(pte | (xferbuf << 5));
}

void *AddrSpace::dtu_handler(m3::ISR::State *state) {
    m3::DTU &dtu = m3::DTU::get();

    // translation request from DTU?
    m3::DTU::reg_t core_req = dtu.get_core_req();
    if(core_req) {
        if(core_req & 0x1)
            PANIC("Unexpected foreign receive: " << m3::fmt(core_req, "x"));
        // acknowledge the translation
        dtu.set_core_req(0);
        handle_xlate(core_req);
    }
    return state;
}
#endif

}
