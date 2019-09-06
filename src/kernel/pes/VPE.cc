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

VPE::VPE(m3::String &&prog, peid_t peid, vpeid_t id, uint flags, KMemObject *kmem,
         epid_t sep, epid_t rep, capsel_t sgate)
    : SListItem(),
      SlabObject<VPE>(),
      RefCounted(),
      _desc(peid, id),
      _flags(flags),
      _pid(),
      _state(DEAD),
      _exitcode(),
      _sysc_ep(SyscallHandler::alloc_ep()),
      _kmem(kmem),
      _name(std::move(prog)),
      _objcaps(id + 1),
      _mapcaps(id + 1),
      _rbufs_size(),
      _upcqueue(*this),
      _vpe_wait_sels(),
      _vpe_wait_count(),
      _as(Platform::pe(pe()).has_virtmem() ? new AddrSpace(pe(), id, sep, rep, sgate) : nullptr),
      _first_sel(m3::KIF::FIRST_FREE_SEL),
      _mem_base() {
    if(_sysc_ep == EP_COUNT)
        PANIC("Too few slots in syscall receive buffers");

    _kmem->alloc(*this, base_kmem());

    _objcaps.set(0, new VPECapability(&_objcaps, 0, this));
    _objcaps.set(1, new MGateCapability(
        &_objcaps, 1, new MGateObject(pe(), id, 0, MEMCAP_END, m3::KIF::Perm::RWX)));
    for(epid_t ep = m3::DTU::FIRST_FREE_EP; ep < EP_COUNT; ++ep) {
        capsel_t sel = m3::KIF::FIRST_EP_SEL + ep - m3::DTU::FIRST_FREE_EP;
        _objcaps.set(sel, new EPCapability(&_objcaps, sel, new EPObject(id, ep)));
    }

    if(Platform::pe(pe()).has_virtmem()) {
        // for the root PT
        _kmem->alloc(*this, PAGE_SIZE);
    }

    // let the VPEManager know about us before we continue with initialization
    VPEManager::get().add(this);

    // we have one reference to ourself
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

bool VPE::invalidate_ep(epid_t ep, bool force) {
    KLOG(EPS, "VPE" << id() << ":EP" << ep << " = invalid");

    if(is_on_pe())
        return DTU::get().inval_ep_remote(desc(), ep, force) == m3::Errors::NONE;
    return true;
}

m3::Errors::Code VPE::config_rcv_ep(epid_t ep, RGateObject &obj) {
    // it needs to be in the receive buffer space
    const goff_t addr = Platform::def_recvbuf(pe());
    const size_t size = Platform::pe(pe()).has_virtmem() ? RECVBUF_SIZE : RECVBUF_SIZE_SPM;
    // def_recvbuf() == 0 means that we do not validate it
    if(addr && (obj.addr < addr || obj.addr > addr + size || obj.addr + obj.size() > addr + size))
        return m3::Errors::INV_ARGS;
    if(obj.addr < addr + _rbufs_size)
        return m3::Errors::INV_ARGS;

    auto pemux = PEManager::get().pemux(pe());

    // no free headers left?
    size_t msgSlots = 1UL << (obj.order - obj.msgorder);
    size_t off = pemux->allocate_headers(msgSlots);
    if(off == m3::DTU::HEADER_COUNT)
        return m3::Errors::OUT_OF_MEM;

    obj.header = off;
    KLOG(EPS, "VPE" << id() << ":EP" << ep << " = "
        "RGate[addr=#" << m3::fmt(obj.addr, "x")
        << ", order=" << obj.order
        << ", msgorder=" << obj.msgorder
        << ", header=" << obj.header
        << "]");

    pemux->dtustate().config_recv(ep, rbuf_base() + obj.addr, obj.order, obj.msgorder, obj.header);
    update_ep(ep);

    m3::ThreadManager::get().notify(reinterpret_cast<event_t>(&obj));
    return m3::Errors::NONE;
}

m3::Errors::Code VPE::config_snd_ep(epid_t ep, SGateObject &obj) {
    assert(obj.rgate->addr != 0);
    if(obj.activated)
        return m3::Errors::EXISTS;

    peid_t peid = VPEManager::get().peof(obj.rgate->vpe);
    KLOG(EPS, "VPE" << id() << ":EP" << ep << " = "
        "Send[vpe=" << obj.rgate->vpe
        << ", pe=" << peid
        << ", ep=" << obj.rgate->ep
        << ", label=#" << m3::fmt(obj.label, "x")
        << ", msgsize=" << obj.rgate->msgorder
        << ", crd=#" << m3::fmt(obj.credits, "x")
        << "]");

    obj.activated = true;
    auto pemux = PEManager::get().pemux(pe());
    pemux->dtustate().config_send(ep, obj.label, peid, obj.rgate->vpe,
                                  obj.rgate->ep, 1UL << obj.rgate->msgorder, obj.credits);
    update_ep(ep);
    return m3::Errors::NONE;
}

m3::Errors::Code VPE::config_mem_ep(epid_t ep, const MGateObject &obj, goff_t off) {
    if(off >= obj.size || obj.addr + off < off)
        return m3::Errors::INV_ARGS;

    KLOG(EPS, "VPE" << id() << ":EP" << ep << " = "
        "Mem [vpe=" << obj.vpe
        << ", pe=" << obj.pe
        << ", addr=#" << m3::fmt(obj.addr + off, "x")
        << ", size=#" << m3::fmt(obj.size - off, "x")
        << ", perms=#" << m3::fmt(obj.perms, "x")
        << "]");

    // TODO
    auto pemux = PEManager::get().pemux(pe());
    pemux->dtustate().config_mem(ep, obj.pe, obj.vpe, obj.addr + off, obj.size - off, obj.perms);
    update_ep(ep);
    return m3::Errors::NONE;
}

void VPE::update_ep(epid_t ep) {
    if(is_on_pe())
        DTU::get().write_ep_remote(desc(), ep, PEManager::get().pemux(pe())->dtustate().get_ep(ep));
}

}
