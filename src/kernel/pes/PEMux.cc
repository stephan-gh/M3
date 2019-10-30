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

#include "pes/PEMux.h"
#include "pes/VPEManager.h"
#include "DTU.h"
#include "Platform.h"
#include "SyscallHandler.h"

#define LOG_PEX_ERROR(pemux, error, msg)                                     \
    do {                                                                 \
        KLOG(ERR, "\e[37;41m"                                            \
            << "PEMux[" << (pemux)->peid() << "]: "                      \
            << msg << " (" << m3::Errors::to_string(error) << ")\e[0m"); \
    }                                                                    \
    while(0)

namespace kernel {

PEMux::PEMux(peid_t pe)
    : _pe(pe, EP_COUNT - m3::DTU::FIRST_FREE_EP),
      _caps(VPE::INVALID_ID),
      _vpes(),
      _reply_eps(EP_COUNT),
      _rbufs_size(),
      _mem_base(),
      _dtustate(),
      _upcqueue(desc()) {
#if defined(__gem5__)
    // configure send EP
    _dtustate.config_send(m3::DTU::KPEX_SEP, reinterpret_cast<label_t>(this),
                          Platform::kernel_pe(), SyscallHandler::pexep(),
                          KPEX_RBUF_ORDER, 1);

    // configure receive EP
    uintptr_t rbuf = Platform::def_recvbuf(peid());
    _dtustate.config_recv(m3::DTU::KPEX_REP, rbuf, KPEX_RBUF_ORDER, KPEX_RBUF_ORDER, _reply_eps);
    rbuf += KPEX_RBUF_SIZE;

    // configure upcall receive EP
    _dtustate.config_recv(m3::DTU::PEXUP_REP, rbuf, PEXUP_RBUF_ORDER, PEXUP_RBUF_ORDER, _reply_eps + 1);
#endif

    for(epid_t ep = m3::DTU::FIRST_USER_EP; ep < EP_COUNT; ++ep) {
        capsel_t sel = m3::KIF::FIRST_EP_SEL + ep - m3::DTU::FIRST_USER_EP;
        _caps.set(sel, new EPCapability(&_caps, sel, new EPObject(&_pe, ep)));
    }

    // one slot for the KPEX receive buffer and one for the upcall receive buffer
    _reply_eps += 2;
}

static void reply_result(const m3::DTU::Message *msg, m3::Errors::Code code) {
    m3::KIF::DefaultReply reply;
    reply.error = static_cast<xfer_t>(code);
    DTU::get().reply(SyscallHandler::pexep(), &reply, sizeof(reply), msg);
}

m3::Errors::Code PEMux::alloc_ep(VPE *caller, vpeid_t dst, capsel_t sel, epid_t *ep) {
    m3::KIF::PEXUpcalls::AllocEP req;
    req.opcode = m3::KIF::PEXUpcalls::ALLOC_EP;
    req.vpe_sel = VPE_SEL_BEGIN + dst;

    KLOG(PEXC, "PEMux[" << peid() << "] sending AllocEP(vpe=" << req.vpe_sel << ")");

    // send upcall
    event_t event = _upcqueue.send(m3::DTU::PEXUP_REP, 0, &req, sizeof(req), false);
    m3::ThreadManager::get().wait_for(event);

    // wait for reply
    auto reply_msg = reinterpret_cast<const m3::DTU::Message*>(m3::ThreadManager::get().get_current_msg());
    auto reply = reinterpret_cast<const m3::KIF::PEXUpcalls::AllocEPReply*>(reply_msg->data);

    KLOG(PEXC, "PEMux[" << peid() << "] got AllocEPReply(error="
        << reply->error << ", ep=" << reply->ep << ")");

    if(reply->error != m3::Errors::NONE)
        return static_cast<m3::Errors::Code>(reply->error);

    capsel_t own_sel = m3::KIF::FIRST_EP_SEL + reply->ep - m3::DTU::FIRST_USER_EP;
    auto epcap = static_cast<EPCapability*>(_caps.get(own_sel, Capability::EP));
    if(epcap == nullptr)
        return m3::Errors::INV_ARGS;
    if(!caller->kmem()->alloc(*caller, sizeof(SharedEPCapability)))
        return m3::Errors::NO_SPACE;

    // create new EP cap for VPE; revocation of that cap will call PEMux::free_ep
    auto aepcap = new SharedEPCapability(&caller->objcaps(), sel, &*epcap->obj);
    caller->objcaps().inherit(epcap, aepcap);
    caller->objcaps().set(sel, aepcap);

    *ep = reply->ep;
    return m3::Errors::NONE;
}

void PEMux::free_ep(epid_t ep) {
    m3::KIF::PEXUpcalls::FreeEP req;
    req.opcode = m3::KIF::PEXUpcalls::FREE_EP;
    req.ep = ep;

    KLOG(PEXC, "PEMux[" << peid() << "] sending FreeEP(ep=" << req.ep << ")");

    _upcqueue.send(m3::DTU::PEXUP_REP, 0, &req, sizeof(req), false);
}

void PEMux::handle_call(const m3::DTU::Message *msg) {
    auto req = reinterpret_cast<const m3::KIF::DefaultRequest*>(msg->data);
    auto op = static_cast<m3::KIF::PEMux::Operation>(req->opcode);

    switch(op) {
        case m3::KIF::PEMux::ACTIVATE:
            pexcall_activate(msg);
            break;

        default:
            reply_result(msg, m3::Errors::INV_ARGS);
            break;
    }
}

void PEMux::pexcall_activate(const m3::DTU::Message *msg) {
    auto req = reinterpret_cast<const m3::KIF::PEMux::Activate*>(msg->data);

    KLOG(PEXC, "PEXCall[" << peid() << "] activate(vpe=" << req->vpe_sel
        << ", gate=" << req->gate_sel << ", ep=" << req->ep
        << ", addr=" << m3::fmt(req->addr, "p") << ")");

    auto vpecap = static_cast<VPECapability*>(_caps.get(req->vpe_sel, Capability::VIRTPE));
    if(vpecap == nullptr) {
        LOG_PEX_ERROR(this, m3::Errors::INV_ARGS, "invalid VPE cap");
        reply_result(msg, m3::Errors::INV_ARGS);
        return;
    }

    capsel_t ep_sel = m3::KIF::FIRST_EP_SEL + req->ep - m3::DTU::FIRST_USER_EP;
    auto epcap = static_cast<EPCapability*>(_caps.get(ep_sel, Capability::EP));
    if(epcap == nullptr) {
        LOG_PEX_ERROR(this, m3::Errors::INV_ARGS, "invalid EP cap");
        reply_result(msg, m3::Errors::INV_ARGS);
        return;
    }

    m3::Errors::Code res = vpecap->obj->activate(epcap, req->gate_sel, req->addr);
    if(res != m3::Errors::NONE)
        LOG_PEX_ERROR(this, res, "activate of EP " << epcap->obj->ep << " failed");
    reply_result(msg, res);
}

size_t PEMux::allocate_reply_eps(size_t num) {
    // TODO really manage the header space and zero the headers first in case they are reused
    if(_reply_eps + num > TOTAL_EPS)
        return TOTAL_EPS;
    _reply_eps += num;
    return _reply_eps - num;
}

bool PEMux::invalidate_ep(epid_t ep, bool force) {
    KLOG(EPS, "PE" << peid() << ":EP" << ep << " = invalid");

    return DTU::get().inval_ep_remote(desc(), ep, force) == m3::Errors::NONE;
}

void PEMux::invalidate_eps() {
    // no update on the PE here, since we don't save the state anyway
    _dtustate.invalidate_eps(m3::DTU::FIRST_FREE_EP);
}

m3::Errors::Code PEMux::config_rcv_ep(epid_t ep, RGateObject &obj) {
    assert(obj.activated());
    // it needs to be in the receive buffer space
    const goff_t addr = Platform::def_recvbuf(peid());
    const size_t size = Platform::pe(peid()).has_virtmem() ? RECVBUF_SIZE : RECVBUF_SIZE_SPM;
    // def_recvbuf() == 0 means that we do not validate it
    if(addr && (obj.addr < addr || obj.addr > addr + size || obj.addr + obj.size() > addr + size))
        return m3::Errors::INV_ARGS;
    if(obj.addr < addr + _rbufs_size)
        return m3::Errors::INV_ARGS;

    // no free headers left?
    size_t msgSlots = 1UL << (obj.order - obj.msgorder);
    size_t off = allocate_reply_eps(msgSlots);
    if(off == TOTAL_EPS)
        return m3::Errors::OUT_OF_MEM;

    obj.header = off;
    KLOG(EPS, "PE" << peid() << ":EP" << ep << " = "
        "RGate[addr=#" << m3::fmt(obj.addr, "x")
        << ", order=" << obj.order
        << ", msgorder=" << obj.msgorder
        << ", header=" << obj.header
        << "]");

    dtustate().config_recv(ep, rbuf_base() + obj.addr, obj.order, obj.msgorder, obj.header);
    update_ep(ep);

    m3::ThreadManager::get().notify(reinterpret_cast<event_t>(&obj));
    return m3::Errors::NONE;
}

m3::Errors::Code PEMux::config_snd_ep(epid_t ep, SGateObject &obj) {
    assert(obj.rgate->addr != 0);
    if(obj.activated)
        return m3::Errors::EXISTS;

    KLOG(EPS, "PE" << peid() << ":EP" << ep << " = "
        "Send[pe=" << obj.rgate->pe
        << ", ep=" << obj.rgate->ep
        << ", label=#" << m3::fmt(obj.label, "x")
        << ", msgsize=" << obj.rgate->msgorder
        << ", crd=#" << m3::fmt(obj.credits, "x")
        << "]");

    obj.activated = true;
    dtustate().config_send(ep, obj.label, obj.rgate->pe, obj.rgate->ep,
                           obj.rgate->msgorder, obj.credits);
    update_ep(ep);
    return m3::Errors::NONE;
}

m3::Errors::Code PEMux::config_mem_ep(epid_t ep, const MGateObject &obj, goff_t off) {
    if(off >= obj.size || obj.addr + off < off)
        return m3::Errors::INV_ARGS;

    KLOG(EPS, "PE" << peid() << ":EP" << ep << " = "
        "Mem [vpe=" << obj.vpe
        << ", pe=" << obj.pe
        << ", addr=#" << m3::fmt(obj.addr + off, "x")
        << ", size=#" << m3::fmt(obj.size - off, "x")
        << ", perms=#" << m3::fmt(obj.perms, "x")
        << "]");

    // TODO
    dtustate().config_mem(ep, obj.pe, obj.addr + off, obj.size - off, obj.perms);
    update_ep(ep);
    return m3::Errors::NONE;
}

void PEMux::update_ep(epid_t ep) {
    DTU::get().write_ep_remote(desc(), ep, dtustate().get_ep(ep));
}

}
