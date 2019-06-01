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

#include "pes/VPEManager.h"
#include "cap/Capability.h"
#include "cap/CapTable.h"
#include "DTU.h"

namespace kernel {

m3::OStream &operator<<(m3::OStream &os, const Capability &cc) {
    cc.print(os);
    return os;
}

KMemObject::KMemObject(size_t _quota)
    : RefCounted(),
      quota(_quota),
      left(_quota) {
    KLOG(KMEM, "KMem[" << m3::fmt((void*)this, "p") << "]: created with " << quota << "");
}

KMemObject::~KMemObject() {
    KLOG(KMEM, "KMem[" << m3::fmt((void*)this, "p") << "]: deleted with " << left << "/" << quota << "");
    assert(left == quota);
}

bool KMemObject::alloc(VPE &vpe, size_t size) {
    KLOG(KMEM_ALLOCS, "KMem[" << m3::fmt((void*)this, "p") << "]: " << vpe.id() << ":" << vpe.name()
      << " allocates " << size << "b (" << left << "/" << quota << " left)");

    if(has_quota(size)) {
        left -= size;
        return true;
    }
    return false;
}

void KMemObject::free(VPE &vpe, size_t size) {
    assert(left + size <= quota);
    left += size;

    KLOG(KMEM_ALLOCS, "KMem[" << m3::fmt((void*)this, "p") << "]: " << vpe.id() << ":" << vpe.name()
      << " freed " << size << "b (" << left << "/" << quota << " left)");
}

GateObject::~GateObject() {
    for(auto user = epuser.begin(); user != epuser.end(); ) {
        auto old = user++;
        VPE &vpe = VPEManager::get().vpe(old->ep->vpe);
        // we want to force-invalidate the send EP if the receive gate is already invalid
        bool force = type == Capability::SGATE && !static_cast<SGateObject*>(this)->rgate_valid();
        vpe.invalidate_ep(old->ep->ep, force);
        old->ep->gate = nullptr;
        delete &*old;
    }
}

RGateObject::~RGateObject() {
    m3::ThreadManager::get().notify(reinterpret_cast<event_t>(this));
}

void SessObject::drop_msgs() {
    srv->drop_msgs(ident);
}

EPObject::~EPObject() {
    if(gate != nullptr)
        gate->remove_ep(this);
}

void KMemCapability::revoke() {
    // grant the kernel memory back to our parent, if there is any
    if(is_root() && parent()) {
        assert(obj->left == obj->quota);
        static_cast<KMemCapability*>(parent())->obj->free(table()->vpe(), obj->left);
    }
}

MapCapability::MapCapability(CapTable *tbl, capsel_t sel, uint _pages, MapObject *_obj)
    : Capability(tbl, sel, MAP, _pages),
      obj(_obj) {
    VPE &vpe = tbl->vpe();
    vpe.address_space()->map_pages(vpe.desc(), sel << PAGE_BITS, obj->phys, length(),
                                   (obj->attr & ~(EXCL | KERNEL)));
}

void MapCapability::remap(gaddr_t _phys, int _attr) {
    obj->phys = _phys;
    obj->attr = _attr;
    VPE &vpe = table()->vpe();
    vpe.address_space()->map_pages(vpe.desc(), sel() << PAGE_BITS, _phys, length(),
                                   (obj->attr & ~(EXCL | KERNEL)));
}

void MapCapability::revoke() {
    VPE &vpe = table()->vpe();
    vpe.address_space()->unmap_pages(vpe.desc(), sel() << PAGE_BITS, length());
    if(obj->attr & EXCL) {
        MainMemory::get().free(MainMemory::get().build_allocation(obj->phys, length() * PAGE_SIZE));
        vpe.kmem()->free(vpe, length() * PAGE_SIZE);
    }
}

void SessCapability::revoke() {
    // drop the queued messages for this session, because the server is not interested anymore
    if(parent()->type() == SERV)
        obj->drop_msgs();
}

void ServCapability::revoke() {
    // first, reset the receive buffer: make all slots not-occupied
    if(obj->rgate()->activated())
        obj->vpe().config_rcv_ep(obj->rgate()->ep, *obj->rgate());
    // now, abort everything in the sendqueue
    obj->abort();
}

size_t VPEGroupCapability::obj_size() const {
    return sizeof(VPEGroup);
}

size_t VPECapability::obj_size() const {
    return sizeof(VPE) + sizeof(AddrSpace);
}

void Capability::print(m3::OStream &os) const {
    os << m3::fmt(table()->id(), 2) << " @ " << m3::fmt(sel(), 6);
    printInfo(os);
    if(_child)
      _child->printChilds(os);
}

void RGateCapability::printInfo(m3::OStream &os) const {
    os << ": rgate[refs=" << obj->refcount()
       << ", ep=" << obj->ep
       << ", addr=#" << m3::fmt(obj->addr, "0x", sizeof(label_t) * 2)
       << ", order=" << obj->order
       << ", msgorder=" << obj->msgorder
       << ", eps=";
    obj->print_eps(os);
    os << "]";
}

void SGateCapability::printInfo(m3::OStream &os) const {
    os << ": sgate[refs=" << obj->refcount()
       << ", dst=" << obj->rgate->vpe << ":" << obj->rgate->ep
       << ", lbl=" << m3::fmt(obj->label, "#0x", sizeof(label_t) * 2)
       << ", crd=#" << m3::fmt(obj->credits, "x")
       << ", eps=";
    obj->print_eps(os);
    os << "]";
}

void MGateCapability::printInfo(m3::OStream &os) const {
    os << ": mgate[refs=" << obj->refcount()
       << ", dst=" << obj->vpe << "@" << obj->pe
       << ", addr=" << m3::fmt(obj->addr, "#0x", sizeof(label_t) * 2)
       << ", size=" << m3::fmt(obj->size, "#0x", sizeof(label_t) * 2)
       << ", perms=#" << m3::fmt(obj->perms, "x")
       << ", eps=";
    obj->print_eps(os);
    os << "]";
}

void MapCapability::printInfo(m3::OStream &os) const {
    os << ": map  [virt=#" << m3::fmt(sel() << PAGE_BITS, "x")
       << ", phys=#" << m3::fmt(obj->phys, "x")
       << ", pages=" << length()
       << ", attr=#" << m3::fmt(obj->attr, "x") << "]";
}

void ServCapability::printInfo(m3::OStream &os) const {
    os << ": serv [name=" << obj->name() << "]";
}

void SessCapability::printInfo(m3::OStream &os) const {
    os << ": sess [refs=" << obj->refcount()
        << ", serv=" << obj->srv->name()
        << ", ident=#" << m3::fmt(obj->ident, "x") << "]";
}

void EPCapability::printInfo(m3::OStream &os) const {
    os << ": ep  [refs=" << obj->refcount()
        << ", vpe=" << obj->vpe
        << ", ep=" << obj->ep << "]";
}

void VPEGroupCapability::printInfo(m3::OStream &os) const {
    os << ": vgrp [refs=" << obj->refcount() << "]";
}

void VPECapability::printInfo(m3::OStream &os) const {
    os << ": vpe  [refs=" << obj->refcount()
       << ", name=" << obj->name() << "]";
}

void KMemCapability::printInfo(m3::OStream &os) const {
    os << ": kmem [refs=" << obj->refcount()
       << ", quota=" << obj->quota
       << ", left=" << obj->left << "]";
}

void Capability::printChilds(m3::OStream &os, size_t layer) const {
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
