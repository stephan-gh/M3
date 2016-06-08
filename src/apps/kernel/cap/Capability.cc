/*
 * Copyright (C) 2015, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include "pes/PEManager.h"
#include "cap/Capability.h"
#include "cap/CapTable.h"

namespace kernel {

m3::OStream &operator<<(m3::OStream &os, const Capability &cc) {
    cc.print(os);
    return os;
}

MemObject::~MemObject() {
    // if it's not derived, it's always memory from mem-PEs
    if(!derived) {
        uintptr_t addr = label & ~m3::KIF::Perm::RWX;
        MainMemory::get().free(core, addr, credits);
    }
}

void SessionObject::close() {
    // only send the close message, if the service has not exited yet
    if(srv->vpe().state() == VPE::RUNNING) {
        AutoGateOStream msg(m3::ostreamsize<m3::KIF::Service::Command, word_t>());
        msg << m3::KIF::Service::CLOSE << ident;
        KLOG(SERV, "Sending CLOSE message for ident " << m3::fmt(ident, "#x", 8)
            << " to " << srv->name());
        ServiceList::get().send_and_receive(srv, msg.bytes(), msg.total());
        msg.claim();
    }
}
SessionObject::~SessionObject() {
    if(!servowned)
        close();
}

m3::Errors::Code MsgCapability::revoke() {
    if(localepid != -1) {
        VPE &vpe = PEManager::get().vpe(table()->id() - 1);
        vpe.xchg_ep(localepid, nullptr, nullptr);
        // wakeup the core to give him the chance to notice that the endpoint was invalidated
        if(vpe.state() != VPE::DEAD)
            DTU::get().wakeup(vpe);
    }
    obj.unref();
    return m3::Errors::NO_ERROR;
}

MapCapability::MapCapability(CapTable *tbl, capsel_t sel, uintptr_t _phys, uint _attr)
    : Capability(tbl, sel, MAP), phys(_phys), attr(_attr) {
    VPE &vpe = PEManager::get().vpe(tbl->id() - 1);
    DTU::get().map_page(vpe, sel << PAGE_BITS, phys, attr);
}

void MapCapability::remap(uint _attr) {
    attr = _attr;
    VPE &vpe = PEManager::get().vpe(table()->id() - 1);
    DTU::get().map_page(vpe, sel() << PAGE_BITS, phys, attr);
}

m3::Errors::Code MapCapability::revoke() {
    VPE &vpe = PEManager::get().vpe(table()->id() - 1);
    DTU::get().unmap_page(vpe, sel() << PAGE_BITS);
    return m3::Errors::NO_ERROR;
}

m3::Errors::Code SessionCapability::revoke() {
    // if the server created that, we want to close it as soon as there are no clients using it anymore
    if(obj->servowned && obj->refcount() == 2)
        obj->close();
    obj.unref();
    return m3::Errors::NO_ERROR;
}

m3::Errors::Code ServiceCapability::revoke() {
    bool closing = inst->closing;
    inst->closing = true;
    // if we have childs, i.e. sessions, we need to send the close-message to the service first,
    // in which case there will be pending requests, which need to be handled first.
    if(inst->pending() > 0 || (child() != nullptr && !closing))
        return m3::Errors::MSGS_WAITING;
    return m3::Errors::NO_ERROR;
}

VPECapability::VPECapability(CapTable *tbl, capsel_t sel, VPE *p)
    : Capability(tbl, sel, VIRTPE), vpe(p) {
    p->ref();
}

VPECapability::VPECapability(const VPECapability &t) : Capability(t), vpe(t.vpe) {
    vpe->ref();
}

m3::Errors::Code VPECapability::revoke() {
    vpe->unref();
    // TODO reset core and release it (make it free to use for others)
    return m3::Errors::NO_ERROR;
}

void MsgCapability::print(m3::OStream &os) const {
    os << m3::fmt(table()->id(), 2) << " @ " << m3::fmt(sel(), 6);
    os << ": mesg[refs=" << obj->refcount()
       << ", curep=" << localepid
       << ", dst=" << obj->core << ":" << obj->epid
       << ", lbl=" << m3::fmt(obj->label, "#0x", sizeof(label_t) * 2)
       << ", crd=#" << m3::fmt(obj->credits, "x") << "]";
    child()->printChilds(os);
}

void MemCapability::print(m3::OStream &os) const {
    os << m3::fmt(table()->id(), 2) << " @ " << m3::fmt(sel(), 6);
    os << ": mem [refs=" << obj->refcount()
       << ", curep=" << localepid
       << ", dst=" << obj->core << ":" << obj->epid << ", lbl=" << m3::fmt(obj->label, "#x")
       << ", crd=#" << m3::fmt(obj->credits, "x") << "]";
    child()->printChilds(os);
}

void MapCapability::print(m3::OStream &os) const {
    os << m3::fmt(table()->id(), 2) << " @ " << m3::fmt(sel(), 6);
    os << ": map [virt=#" << m3::fmt(sel() << PAGE_BITS, "x")
       << ", phys=#" << m3::fmt(phys, "x")
       << ", attr=#" << m3::fmt(attr, "x") << "]";
    child()->printChilds(os);
}

void ServiceCapability::print(m3::OStream &os) const {
    os << m3::fmt(table()->id(), 2) << " @ " << m3::fmt(sel(), 6);
    os << ": serv[name=" << inst->name() << "]";
    child()->printChilds(os);
}

void SessionCapability::print(m3::OStream &os) const {
    os << m3::fmt(table()->id(), 2) << " @ " << m3::fmt(sel(), 6);
    os << ": sess[refs=" << obj->refcount()
        << ", serv=" << obj->srv->name()
        << ", ident=#" << m3::fmt(obj->ident, "x")
        << ", servowned=" << obj->servowned << "]";
    child()->printChilds(os);
}

void VPECapability::print(m3::OStream &os) const {
    os << m3::fmt(table()->id(), 2) << " @ " << m3::fmt(sel(), 6);
    os << ": vpe [refs=" << vpe->refcount()
       << ", name=" << vpe->name() << "]";
    child()->printChilds(os);
}

void Capability::printChilds(m3::OStream &os, int layer) const {
    const Capability *n = this;
    while(n) {
        os << "\n";
        os << m3::fmt("", layer * 2) << " \\-";
        n->print(os);
        if(n->_child)
            n->_child->printChilds(os, layer + 1);
        n = n->_next;
    }
}

}
