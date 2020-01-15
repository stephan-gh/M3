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
#include "Args.h"
#include "DTU.h"
#include "Platform.h"
#include "SyscallHandler.h"

namespace kernel {

VPE::VPE(m3::String &&prog, PECapability *pecap, vpeid_t id, uint flags, KMemCapability *kmemcap)
    : SlabObject<VPE>(),
      RefCounted(),
      _desc(pecap ? pecap->obj->id : 1, id),
      _flags(flags),
      _pid(),
      _state(DEAD),
      _exitcode(),
      _sysc_ep(SyscallHandler::alloc_ep()),
      _kmem(kmemcap ? kmemcap->obj : m3::Reference<KMemObject>()),
      _pe(pecap ? pecap->obj : m3::Reference<PEObject>()),
      _eps(),
      _pg_sep(),
      _pg_rep(),
      _name(std::move(prog)),
      _objcaps(id),
      _mapcaps(id),
      _upcqueue(desc()),
      _vpe_wait_sels(),
      _vpe_wait_count(),
      _first_sel(m3::KIF::FIRST_FREE_SEL) {
    if(_sysc_ep == EP_COUNT)
        PANIC("Too few slots in syscall receive buffers");

    auto vpecap = new VPECapability(&_objcaps, m3::KIF::SEL_VPE, this);
    _objcaps.set(m3::KIF::SEL_VPE, vpecap);

    // allocate PE cap for root
    if(pecap == nullptr) {
        pecap = new PECapability(&_objcaps, m3::KIF::SEL_PE, PEManager::get().pemux(peid())->pe());
        _objcaps.set(m3::KIF::SEL_PE, pecap);
        _pe = pecap->obj;

        // same for kmem
        assert(kmemcap == nullptr);
        auto kmem = new KMemObject(Args::kmem - FIXED_KMEM);
        kmemcap = new KMemCapability(&_objcaps, m3::KIF::SEL_KMEM, kmem);
        _objcaps.set(m3::KIF::SEL_KMEM, kmemcap);
        _kmem = kmemcap->obj;

        // KMemCapability and PECapability are already paid by base_kmem()
        _kmem->alloc(*this, sizeof(KMemObject) + sizeof(PEObject));
    }
    else {
        auto npecap = pecap->clone(&_objcaps, m3::KIF::SEL_PE);
        _objcaps.inherit(pecap, npecap);
        _objcaps.set(m3::KIF::SEL_PE, npecap);
        // same for kmem
        assert(kmemcap != nullptr);
        auto nkmemcap = kmemcap->clone(&_objcaps, m3::KIF::SEL_KMEM);
        _objcaps.inherit(kmemcap, nkmemcap);
        _objcaps.set(m3::KIF::SEL_KMEM, nkmemcap);
    }

    _kmem->alloc(*this, required_kmem());

    _objcaps.set(m3::KIF::SEL_MEM, new MGateCapability(
        &_objcaps, m3::KIF::SEL_MEM, new MGateObject(peid(), 0, MEMCAP_END, m3::KIF::Perm::RWX)));

    // let the VPEManager know about us before we continue with initialization
    VPEManager::get().add(vpecap);
    _pe->vpes++;

    // we have one reference to ourself
    rem_ref();
    // and PEMux has one reference to us
    rem_ref();

    if(!Platform::pe(peid()).is_device())
        init_eps();

    KLOG(VPES, "Created VPE '" << _name << "' [id=" << id << ", pe=" << peid() << "]");
}

VPE::~VPE() {
    KLOG(VPES, "Deleting VPE '" << _name << "' [id=" << id() << "]");

    _state = DEAD;

    // ensure that the VPE is stopped
    PEManager::get().stop_vpe(this);

    _objcaps.revoke_all();
    _mapcaps.revoke_all();

    // ensure that there are no syscalls for this VPE anymore
    m3::DTU::get().drop_msgs(syscall_ep(), m3::ptr_to_label(this));
    SyscallHandler::free_ep(syscall_ep());

    delete _pg_sep;
    delete _pg_rep;

    _pe->vpes--;
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
        // ensure that there are no pending system calls
        m3::DTU::get().drop_msgs(syscall_ep(), m3::ptr_to_label(this));
        if(_state == RUNNING) {
            // device always exit successfully
            exitcode = Platform::pe(peid()).is_device() ? 0 : 1;
            exit_app(exitcode);
        }
        else {
            _flags ^= F_HASAPP;
            PEManager::get().stop_vpe(this);
        }
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
    auto pemux = PEManager::get().pemux(peid());
    for(auto ep = _eps.begin(); ep != _eps.end(); ++ep) {
        if(ep->gate != nullptr) {
            pemux->invalidate_ep(ep->ep);
            if(ep->gate->type == Capability::SGATE)
                static_cast<SGateObject*>(ep->gate)->activated = false;
            else if(ep->gate->type == Capability::RGATE) {
                static_cast<RGateObject*>(ep->gate)->addr = 0;
                static_cast<RGateObject*>(ep->gate)->valid = false;
            }

            // forget the connection
            ep->gate->remove_ep(&*ep);
            ep->gate = nullptr;
        }
        ep->vpe = nullptr;
    }
    _eps.clear();

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
    PEManager::get().pemux(peid())->set_mem_base(addr);
    finish_start();
}

void VPE::update_ep(epid_t ep) {
    if(is_on_pe())
        DTU::get().write_ep_remote(desc(), ep, PEManager::get().pemux(peid())->dtustate().get_ep(ep));
}

}
