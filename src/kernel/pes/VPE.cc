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

#include <base/Common.h>
#include <base/log/Kernel.h>
#include <base/util/Math.h>
#include <base/util/Time.h>

#include <thread/ThreadManager.h>

#include <utility>

#include "pes/PEManager.h"
#include "pes/VPEManager.h"
#include "pes/VPE.h"
#include "DTU.h"
#include "Platform.h"
#include "SyscallHandler.h"

namespace kernel {

VPE::VPE(m3::String &&prog, peid_t peid, vpeid_t id, uint flags, KMemObject *kmem)
    : SlabObject<VPE>(),
      RefCounted(),
      _desc(peid, id),
      _flags(flags),
      _pid(),
      _state(DEAD),
      _exitcode(),
      _sysc_ep(SyscallHandler::alloc_ep()),
      _kmem(kmem),
      _name(std::move(prog)),
      _objcaps(id),
      _mapcaps(id),
      _upcqueue(desc()),
      _vpe_wait_sels(),
      _vpe_wait_count(),
      _as(Platform::pe(pe()).has_virtmem() ? new AddrSpace(pe(), id) : nullptr),
      _first_sel(m3::KIF::FIRST_FREE_SEL) {
    if(_sysc_ep == EP_COUNT)
        PANIC("Too few slots in syscall receive buffers");

    _kmem->alloc(*this, base_kmem());

    auto vpecap = new VPECapability(&_objcaps, 0, this);
    _objcaps.set(0, vpecap);
    _objcaps.set(1, new MGateCapability(
        &_objcaps, 1, new MGateObject(pe(), id, 0, MEMCAP_END, m3::KIF::Perm::RWX)));

    // only accelerators get their EP caps directly, because no PEMux is running there
    if(!USE_PEMUX || !Platform::pe(pe()).is_programmable()) {
        for(epid_t ep = m3::DTU::FIRST_FREE_EP; ep < EP_COUNT; ++ep) {
            capsel_t sel = m3::KIF::FIRST_EP_SEL + ep - m3::DTU::FIRST_FREE_EP;
            _objcaps.set(sel, new EPCapability(&_objcaps, sel, new EPObject(pe(), ep)));
        }
    }

    if(Platform::pe(pe()).has_virtmem()) {
        // for the root PT
        _kmem->alloc(*this, PAGE_SIZE);
    }

    // let the VPEManager know about us before we continue with initialization
    VPEManager::get().add(vpecap);

    // we have one reference to ourself
    rem_ref();
    // and PEMux has one reference to us
    rem_ref();

    init_eps();

    KLOG(VPES, "Created VPE '" << _name << "' [id=" << id << ", pe=" << pe() << "]");
}

VPE::~VPE() {
    KLOG(VPES, "Deleting VPE '" << _name << "' [id=" << id() << "]");

    _state = DEAD;

    // ensure that the VPE is stopped
    PEManager::get().stop_vpe(this);

    _objcaps.revoke_all();
    _mapcaps.revoke_all();

    // ensure that there are no syscalls for this VPE anymore
    m3::DTU::get().drop_msgs(syscall_ep(), reinterpret_cast<label_t>(this));
    SyscallHandler::free_ep(syscall_ep());

    delete _as;

    VPEManager::get().remove(this);
}

void VPE::start_app(int pid) {
    if(has_app())
        return;

    _pid = pid;
    _flags |= F_HASAPP;

    // when exiting, the program will release one reference
    add_ref();

    KLOG(VPES, "Starting VPE '" << _name << "' [id=" << id() << "]");

    PEManager::get().start_vpe(this);
}

void VPE::stop_app(int exitcode, bool self) {
    if(!has_app())
        return;

    KLOG(VPES, "Stopping VPE '" << _name << "' [id=" << id() << "]");

    if(self)
        exit_app(exitcode);
    else {
        if(_state == RUNNING)
            exit_app(1);
        else {
            PEManager::get().stop_vpe(this);
            _flags ^= F_HASAPP;
        }
        // ensure that there are no pending system calls
        m3::DTU::get().drop_msgs(syscall_ep(), reinterpret_cast<label_t>(this));
    }

    if(rem_ref())
        delete this;
}

static int exit_event = 0;

void VPE::wait_for_exit() {
    m3::ThreadManager::get().wait_for(reinterpret_cast<event_t>(&exit_event));
    m3::CPU::compiler_barrier();
}

void VPE::exit_app(int exitcode) {
    PEManager::get().pemux(pe())->invalidate_eps();

    // "deactivate" send and receive gates
    for(capsel_t sel = m3::KIF::FIRST_EP_SEL; sel < m3::KIF::FIRST_FREE_SEL; ++sel) {
        auto epcap = static_cast<EPCapability*>(_objcaps.get(sel, Capability::EP));
        if(epcap == nullptr || epcap->obj->gate == nullptr)
            continue;

        if(epcap->obj->gate->type == Capability::SGATE)
            static_cast<SGateObject*>(epcap->obj->gate)->activated = false;
        else if(epcap->obj->gate->type == Capability::RGATE) {
            static_cast<RGateObject*>(epcap->obj->gate)->addr = 0;
            static_cast<RGateObject*>(epcap->obj->gate)->valid = false;
        }

        // forget the connection
        epcap->obj->gate->remove_ep(&*epcap->obj);
        epcap->obj->gate = nullptr;
    }

    _exitcode = exitcode;

    _flags ^= F_HASAPP;

    PEManager::get().stop_vpe(this);

    m3::ThreadManager::get().notify(reinterpret_cast<event_t>(&exit_event));
}

bool VPE::check_exits(const xfer_t *sels, size_t count, m3::KIF::Syscall::VPEWaitReply &reply) {
    for(size_t i = 0; i < count; ++i) {
        auto vpecap = static_cast<VPECapability*>(_objcaps.get(sels[i], Capability::VIRTPE));
        if(vpecap == nullptr || &*vpecap->obj == this)
            continue;

        if(!vpecap->obj->has_app()) {
            reply.vpe_sel = sels[i];
            reply.exitcode = static_cast<xfer_t>(vpecap->obj->exitcode());
            return true;
        }
    }

    VPE::wait_for_exit();
    return false;
}

void VPE::wait_exit_async(xfer_t *sels, size_t count, m3::KIF::Syscall::VPEWaitReply &reply) {
    _vpe_wait_count = count;
    // remember the location for later modification
    if(!_vpe_wait_sels)
        _vpe_wait_sels = sels;
    else {
        // update the selectors and return
        memcpy(const_cast<xfer_t*>(_vpe_wait_sels), sels, count * sizeof(xfer_t));
        return;
    }

    while(!check_exits(const_cast<const xfer_t*>(_vpe_wait_sels), _vpe_wait_count, reply))
        ;

    _vpe_wait_sels = nullptr;
}

void VPE::wakeup() {
    DTU::get().inject_irq(desc());
}

void VPE::upcall_vpewait(word_t event, m3::KIF::Syscall::VPEWaitReply &reply) {
    m3::KIF::Upcall::VPEWait msg;
    msg.opcode = m3::KIF::Upcall::VPEWAIT;
    msg.event = event;
    msg.error = reply.error;
    msg.vpe_sel = reply.vpe_sel;
    msg.exitcode = reply.exitcode;
    KLOG(UPCALLS, "Sending upcall VPEWAIT (error=" << reply.error << ", event="
        << (void*)event << ", sel=" << reply.vpe_sel << ", exitcode=" << reply.exitcode << ") to VPE " << id());
    upcall(&msg, sizeof(msg), false);
}

void VPE::set_mem_base(goff_t addr) {
    PEManager::get().pemux(pe())->set_mem_base(addr);
    finish_start();
}

m3::Errors::Code VPE::activate(EPCapability *epcap, capsel_t gate, size_t addr) {
    peid_t dst_pe = epcap->obj->pe;
    PEMux *dst_pemux = PEManager::get().pemux(dst_pe);

    GateObject *gateobj = nullptr;
    if(gate != m3::KIF::INV_SEL) {
        auto gatecap = objcaps().get(gate, Capability::SGATE | Capability::MGATE | Capability::RGATE);
        if(gatecap == nullptr)
            return m3::Errors::INV_ARGS;
        gateobj = gatecap->as_gate();
    }

    bool invalid = false;
    if(epcap->obj->gate) {
        if(epcap->obj->gate->type == Capability::RGATE)
            static_cast<RGateObject*>(epcap->obj->gate)->addr = 0;
        // the remote invalidation is only required for send gates
        else if(epcap->obj->gate->type == Capability::SGATE) {
            if(!dst_pemux->invalidate_ep(epcap->obj->ep))
                return m3::Errors::INV_ARGS;
            static_cast<SGateObject*>(epcap->obj->gate)->activated = false;
            invalid = true;
        }

        if(gateobj != epcap->obj->gate) {
            epcap->obj->gate->remove_ep(&*epcap->obj);
            epcap->obj->gate = nullptr;
        }
    }

    if(gateobj) {
        EPObject *oldep = gateobj->ep_of_pe(dst_pe);
        if(oldep && oldep->ep != epcap->obj->ep)
            return m3::Errors::EXISTS;

        if(gateobj->type == Capability::MGATE) {
            auto mgateobj = static_cast<MGateObject*>(gateobj);
            m3::Errors::Code res = dst_pemux->config_mem_ep(epcap->obj->ep, *mgateobj, addr);
            if(res != m3::Errors::NONE)
                return res;
        }
        else if(gateobj->type == Capability::SGATE) {
            auto sgateobj = static_cast<SGateObject*>(gateobj);

            if(!sgateobj->rgate->activated()) {
                // LOG_SYS(vpe, ": syscall::activate",
                //     ": waiting for rgate " << &sgateobj->rgate);

                m3::ThreadManager::get().wait_for(reinterpret_cast<event_t>(&*sgateobj->rgate));

                // LOG_SYS(vpe, ": syscall::activate-cont",
                //     ": rgate " << &sgateobj->rgate << " activated");

                // ensure that dstvpe is still valid
                // TODO how to handle that?
                // if(objcaps().get(ep, Capability::EP) == nullptr)
                //     return m3::Errors::INV_ARGS;
            }

            m3::Errors::Code res = dst_pemux->config_snd_ep(epcap->obj->ep, *sgateobj);
            if(res != m3::Errors::NONE)
                return res;
        }
        else {
            auto rgateobj = static_cast<RGateObject*>(gateobj);
            if(rgateobj->activated())
                return m3::Errors::EXISTS;

            rgateobj->pe = dst_pe;
            rgateobj->addr = addr;
            rgateobj->ep = epcap->obj->ep;

            m3::Errors::Code res = dst_pemux->config_rcv_ep(epcap->obj->ep, *rgateobj);
            if(res != m3::Errors::NONE) {
                rgateobj->addr = 0;
                return res;
            }
        }

        if(!oldep)
            gateobj->add_ep(&*epcap->obj);
    }
    else {
        if(!invalid && !dst_pemux->invalidate_ep(epcap->obj->ep))
            return m3::Errors::INV_ARGS;
    }

    epcap->obj->gate = gateobj;
    return m3::Errors::NONE;
}

void VPE::update_ep(epid_t ep) {
    if(is_on_pe())
        DTU::get().write_ep_remote(desc(), ep, PEManager::get().pemux(pe())->dtustate().get_ep(ep));
}

}
