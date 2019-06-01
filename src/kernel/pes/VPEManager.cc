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

#include <base/log/Kernel.h>
#include <base/util/Math.h>
#include <base/Panic.h>

#include "pes/PEManager.h"
#include "pes/VPEManager.h"
#include "Args.h"
#include "Platform.h"
#include "WorkLoop.h"

namespace kernel {

VPEManager *VPEManager::_inst;

VPEManager::VPEManager()
    : _next_id(0),
      _vpes(new VPE*[MAX_VPES]()),
      _count() {
}

KMemObject *VPEManager::start_root() {
    // TODO the required PE depends on the boot module, not the kernel PE
    m3::PEDesc pedesc = Platform::pe(Platform::kernel_pe());
    m3::PEDesc pedesc_emem(m3::PEType::COMP_EMEM, pedesc.isa(), pedesc.mem_size());
    m3::PEDesc pedesc_imem(m3::PEType::COMP_IMEM, pedesc.isa(), pedesc.mem_size());

    vpeid_t id = get_id();
    assert(id != MAX_VPES);

    // try to find a PE with the required ISA and external memory first
    peid_t peid = PEManager::get().find_pe(pedesc_emem, 0, 0, nullptr);
    if(peid == 0) {
        // if that failed, try to find a SPM PE
        peid = PEManager::get().find_pe(pedesc_imem, 0, 0, nullptr);
        if(peid == 0)
            PANIC("Unable to find a free PE for root task");
    }

    auto kmem = new KMemObject(Args::kmem - FIXED_KMEM);
    _vpes[id] = new VPE("root", peid, id, VPE::F_BOOTMOD, kmem);

    capsel_t sel = m3::KIF::FIRST_FREE_SEL;

    // kernel memory
    auto kmemcap = new KMemCapability(&_vpes[id]->objcaps(), sel, kmem);
    _vpes[id]->objcaps().set(sel, kmemcap);
    kmem->alloc(*_vpes[id], sizeof(KMemCapability) + sizeof(KMemObject));
    sel++;

    // boot info
    {
        peid_t pe = m3::DTU::gaddr_to_pe(Platform::info_addr());
        goff_t addr = m3::DTU::gaddr_to_virt(Platform::info_addr());
        auto memcap = CREATE_CAP(MGateCapability, MGateObject,
            &_vpes[id]->objcaps(), sel,
            pe, VPE::INVALID_ID, addr, Platform::info_size(), m3::KIF::Perm::R
        );
        _vpes[id]->objcaps().set(sel, memcap);
        sel++;
    }

    // boot modules
    for(auto mod = Platform::mods_begin(); mod != Platform::mods_end(); ++mod, ++sel) {
        peid_t pe = m3::DTU::gaddr_to_pe(mod->addr);
        goff_t addr = m3::DTU::gaddr_to_virt(mod->addr);
        size_t size = m3::Math::round_up(static_cast<size_t>(mod->size),
                                         static_cast<size_t>(PAGE_SIZE));
        auto memcap = CREATE_CAP(MGateCapability, MGateObject,
            &_vpes[id]->objcaps(), sel,
            pe, VPE::INVALID_ID, addr, size, m3::KIF::Perm::R | m3::KIF::Perm::X
        );
        _vpes[id]->objcaps().set(sel, memcap);
    }

    // memory
    for(size_t i = 0; i < MainMemory::get().mod_count(); ++i) {
        const MemoryModule &mod = MainMemory::get().module(i);
        if(mod.type() != MemoryModule::KERNEL) {
            auto memcap = CREATE_CAP(MGateCapability, MGateObject,
                &_vpes[id]->objcaps(), sel,
                mod.pe(), VPE::INVALID_ID, mod.addr(), mod.size(), m3::KIF::Perm::RWX
            );
            _vpes[id]->objcaps().set(sel, memcap);
            sel++;
        }
    }

    // go!
    _vpes[id]->set_first_sel(sel);
    _vpes[id]->start_app(_vpes[id]->pid());

    return kmem;
}

vpeid_t VPEManager::get_id() {
    vpeid_t id = _next_id;
    for(; id < MAX_VPES && _vpes[id] != nullptr; ++id)
        ;
    if(id == MAX_VPES) {
        for(id = 0; id < MAX_VPES && _vpes[id] != nullptr; ++id)
            ;
    }
    if(id == MAX_VPES)
        return MAX_VPES;
    _next_id = id + 1;
    return id;
}

VPE *VPEManager::create(m3::String &&name, const m3::PEDesc &pe, epid_t sep, epid_t rep,
                        capsel_t sgate, KMemObject *kmem, uint flags, VPEGroup *group) {
    uint vflags = 0;
    if(flags & m3::KIF::VPEFlags::MUXABLE)
        vflags |= VPE::F_MUXABLE;
    if(flags & m3::KIF::VPEFlags::PINNED)
        vflags |= VPE::F_PINNED;

    peid_t i = PEManager::get().find_pe(pe, 0, vflags, group);
    if(i == 0)
        return nullptr;

    // a pager without virtual memory support, doesn't work
    if(!Platform::pe(i).has_virtmem() && sgate != m3::KIF::INV_SEL)
        return nullptr;

    vpeid_t id = get_id();
    if(id == MAX_VPES)
        return nullptr;

    // groups are implicitly pinned
    if(group)
        vflags |= VPE::F_PINNED;
    VPE *vpe = new VPE(m3::Util::move(name), i, id, vflags, kmem, sep, rep, sgate, group);
    assert(vpe == _vpes[id]);

    return vpe;
}

void VPEManager::add(VPE *vpe) {
    _vpes[vpe->id()] = vpe;

    if(~vpe->_flags & VPE::F_IDLE) {
        _count++;
        PEManager::get().add_vpe(vpe);
    }
}

void VPEManager::remove(VPE *vpe) {
    PEManager::get().remove_vpe(vpe);

    // do that afterwards, because some actions in the destructor might try to get the VPE
    _vpes[vpe->id()] = nullptr;

    if(vpe->_flags & VPE::F_IDLE)
        return;

    assert(_count > 0);
    _count--;

    // if there are no VPEs left, we can stop everything
    if(_count == 0)
        WorkLoop::get().stop();
}

}
