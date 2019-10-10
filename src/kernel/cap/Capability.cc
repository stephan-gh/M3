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

#include <thread/ThreadManager.h>

#include "pes/PEMux.h"
#include "pes/PEManager.h"
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

void PEObject::alloc(uint eps) {
    KLOG(PES, "PE[" << id << "]: allocating " << eps << " EPs (" << this->eps << " total)");
    assert(this->eps >= eps);
    this->eps -= eps;
}

void PEObject::free(uint eps) {
    this->eps += eps;
    KLOG(PES, "PE[" << id << "]: freed " << eps << " EPs (" << this->eps << " total)");
}

GateObject::~GateObject() {
    for(auto user = epuser.begin(); user != epuser.end(); ) {
        auto old = user++;
        PEMux *pemux = PEManager::get().pemux(old->ep->pe->id);
        // always force-invalidate send gates here
        pemux->invalidate_ep(old->ep->ep, type == Capability::SGATE);
        // invalidate reply caps at receiver
        if(type == Capability::SGATE && static_cast<SGateObject*>(this)->rgate_valid()) {
            auto sgate = static_cast<SGateObject*>(this);
            PEMux *receiver = PEManager::get().pemux(sgate->rgate->pe);
            KLOG(EPS, "PE" << pemux->pe() << ":EP" << old->ep->ep << ": invalidating reply caps at "
                   << "PE" << receiver->pe() << ":EP" << sgate->rgate->ep);
            DTU::get().inv_reply_remote(receiver->desc(), sgate->rgate->ep, pemux->peid(), old->ep->ep);
        }
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

m3::Errors::Code SemObject::down() {
    while(*const_cast<volatile uint*>(&counter) == 0) {
        waiters++;
        // TODO prevent starvation
        m3::ThreadManager::get().wait_for(reinterpret_cast<event_t>(this));
        if(*const_cast<volatile int*>(&waiters) == -1)
            return m3::Errors::RECV_GONE;
        waiters--;
    }
    counter--;
    return m3::Errors::NONE;
}

void SemObject::up() {
    if(waiters > 0)
        m3::ThreadManager::get().notify(reinterpret_cast<event_t>(this));
    counter++;
}

void SemCapability::revoke() {
    if(obj->waiters > 0)
        m3::ThreadManager::get().notify(reinterpret_cast<event_t>(&*obj));
    obj->waiters = -1;
}

void KMemCapability::revoke() {
    // grant the kernel memory back to our parent, if there is any
    if(is_root() && parent()) {
        auto *vpe = table()->vpe();
        assert(vpe != nullptr);
        assert(obj->left == obj->quota);
        static_cast<KMemCapability*>(parent())->obj->free(*vpe, obj->left);
    }
}

void PECapability::revoke() {
    // grant the EPs back to our parent, if there is any
    if(is_root() && parent())
        static_cast<PECapability*>(parent())->obj->free(obj->eps);
}

void SharedEPCapability::revoke() {
    // free PE at PEMux
    auto pemux = PEManager::get().pemux(obj->pe->id);
    pemux->free_ep(obj->ep);

    // grant it back to PE cap
    obj->pe->free(1);
}

MapCapability::MapCapability(CapTable *tbl, capsel_t sel, uint _pages, MapObject *_obj)
    : Capability(tbl, sel, MAP, _pages),
      obj(_obj) {
    VPE *vpe = tbl->vpe();
    assert(vpe != nullptr);
    vpe->address_space()->map_pages(vpe->desc(), sel << PAGE_BITS, obj->phys, length(),
                                    (obj->attr & ~(EXCL | KERNEL)));
}

void MapCapability::remap(gaddr_t _phys, int _attr) {
    obj->phys = _phys;
    obj->attr = _attr;
    VPE *vpe = table()->vpe();
    assert(vpe != nullptr);
    vpe->address_space()->map_pages(vpe->desc(), sel() << PAGE_BITS, _phys, length(),
                                    (obj->attr & ~(EXCL | KERNEL)));
}

void MapCapability::revoke() {
    VPE *vpe = table()->vpe();
    assert(vpe != nullptr);
    vpe->address_space()->unmap_pages(vpe->desc(), sel() << PAGE_BITS, length());
    if(obj->attr & EXCL) {
        MainMemory::get().free(MainMemory::get().build_allocation(obj->phys, length() * PAGE_SIZE));
        vpe->kmem()->free(*vpe, length() * PAGE_SIZE);
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
        PEManager::get().pemux(obj->vpe().pe())->config_rcv_ep(obj->rgate()->ep, *obj->rgate());
    // now, abort everything in the sendqueue
    obj->abort();
}

size_t VPECapability::obj_size() const {
    return sizeof(VPE) + sizeof(AddrSpace);
}

void Capability::print(m3::OStream &os) const {
    os << m3::fmt(table()->vpeid(), 2) << " @ " << m3::fmt(sel(), 6);
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
       << ", dst=" << obj->rgate->pe << ":" << obj->rgate->ep
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

void PECapability::printInfo(m3::OStream &os) const {
    os << ": pe  [refs=" << obj->refcount()
        << ", pe=" << obj->id
        << ", eps=" << obj->eps << "]";
}

void EPCapability::printInfo(m3::OStream &os) const {
    os << ": ep  [refs=" << obj->refcount()
        << ", pe=" << obj->pe->id
        << ", ep=" << obj->ep << "]";
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

void SemCapability::printInfo(m3::OStream &os) const {
    os << ": sem [refs=" << obj->refcount()
       << ", counter=" << obj->counter
       << ", waiters=" << obj->waiters << "]";
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
