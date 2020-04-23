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
#include "pes/VPE.h"
#include "cap/Capability.h"
#include "cap/CapTable.h"
#include "mem/MainMemory.h"
#include "TCU.h"

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

void GateObject::revoke(bool foreign) {
    for(auto user = epuser.begin(); user != epuser.end(); ) {
        auto old = user++;
        PEMux *pemux = PEManager::get().pemux(old->ep->pe->id);
        // always force-invalidate send gates here
        pemux->invalidate_ep(old->ep->vpe->id(), old->ep->ep, type == Capability::SGATE);
        // notify PEMux about the invalidation if it's not a self-invalidation (technically,
        // <foreign> indicates whether we're in the first level of revoke, but since it is just a
        // notification, we can ignore the case that someone delegated a cap to itself).
        if(type == Capability::SGATE && foreign)
            pemux->notify_invalidate(old->ep->vpe->id(), old->ep->ep);
        // invalidate reply caps at receiver
        if(type == Capability::SGATE && static_cast<SGateObject*>(this)->rgate_valid()) {
            auto sgate = static_cast<SGateObject*>(this);
            PEMux *receiver = PEManager::get().pemux(sgate->rgate->pe);
            KLOG(EPS, "PE" << pemux->peid() << ":EP" << old->ep->ep << ": invalidating reply caps at "
                   << "PE" << receiver->peid() << ":EP" << sgate->rgate->ep);
            TCU::inv_reply_remote(receiver->peid(), sgate->rgate->ep, pemux->peid(), old->ep->ep);
        }
        old->ep->gate = nullptr;
        delete &*old;
    }
}

// done in revoke instead of ~RGateObject, because GateObject::revoke() needs to be interruptable.
void RGateCapability::revoke(bool foreign) {
    if(is_root()) {
        // mark it as invalid to force-invalidate its send gates
        obj->valid = false;
        obj->revoke(foreign);
        m3::ThreadManager::get().notify(reinterpret_cast<event_t>(this));
    }
}

void SessObject::drop_msgs() {
    srv->drop_msgs(ident);
}

EPObject::EPObject(PEObject *_pe, bool _is_std, VPE *_vpe, epid_t _ep, uint _replies)
    : RefCounted(),
      DListItem(),
      is_std(_is_std),
      vpe(_vpe),
      ep(_ep),
      replies(_replies),
      pe(_pe),
      gate() {
    vpe->add_ep(this);
}

EPObject::~EPObject() {
    if(gate != nullptr)
        gate->remove_ep(this);

    if(vpe != nullptr)
        vpe->remove_ep(this);

    // this check is necessary for the pager EP objects in the VPE
    if(!is_std) {
        // free EPs at PEMux
        auto pemux = PEManager::get().pemux(pe->id);
        pemux->free_eps(ep, 1 + replies);

        // grant it back to PE cap
        pe->free(1 + replies);
    }
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

SemObject::~SemObject() {
    if(waiters > 0)
        m3::ThreadManager::get().notify(reinterpret_cast<event_t>(this));
    waiters = -1;
}

// done in revoke instead of ~KMemObject, because we need access to the parent cap. this is okay,
// because we only do that for the root capability, which makes it equivalent to performing the
// action in ~KMemObject.
void KMemCapability::revoke(bool) {
    // grant the kernel memory back to our parent, if there is any
    if(is_root() && parent()) {
        auto *vpe = table()->vpe();
        assert(vpe != nullptr);
        assert(obj->left == obj->quota);
        static_cast<KMemCapability*>(parent())->obj->free(*vpe, obj->left);
    }
}

// same as above
void PECapability::revoke(bool) {
    // grant the EPs back to our parent, if there is any
    if(is_root() && parent())
        static_cast<PECapability*>(parent())->obj->free(obj->eps);
}

m3::Errors::Code MapCapability::remap(m3::GlobAddr _global, uint _attr) {
    VPE *vpe = table()->vpe();
    assert(vpe != nullptr);
    auto pemux = PEManager::get().pemux(vpe->peid());
    auto perms = _attr & ~(EXCL | KERNEL);
    m3::Errors::Code res = pemux->map(vpe->id(), sel() << PAGE_BITS, _global, length(), perms);
    if(res != m3::Errors::NONE)
      return res;

    obj->global = _global;
    obj->attr = _attr;
    return m3::Errors::NONE;
}

// done in revoke instead of ~MapObject, because we need access to the VPE. this is okay, because
// MapCapability cannot be cloned anyway.
void MapCapability::revoke(bool) {
    VPE *vpe = table()->vpe();
    assert(vpe != nullptr);
    if(!vpe->is_stopped()) {
        auto pemux = PEManager::get().pemux(vpe->peid());
        pemux->map(vpe->id(), sel() << PAGE_BITS, m3::GlobAddr(), length(), 0);
    }
    if(obj->attr & EXCL) {
        MainMemory::get().free(MainMemory::get().build_allocation(obj->global, length() * PAGE_SIZE));
        vpe->kmem()->free(*vpe, length() * PAGE_SIZE);
    }
}

// done in revoke instead of in ~SessObject, because we want to perform the action as soon as the
// client's session capability is revoked.
void SessCapability::revoke(bool) {
    // drop the queued messages for this session, because the server is not interested anymore
    if(parent()->type() == SERV)
        obj->drop_msgs();
    else if(parent()->is_root() && !parent()->in_revocation() &&
            static_cast<SessCapability*>(parent())->obj->auto_close) {
        m3::KIF::Service::Close smsg;
        smsg.opcode = m3::KIF::Service::CLOSE;
        smsg.sess = obj->ident;
        KLOG(SERV, "Sending CLOSE to service " << obj->srv->name()
            << " for sess " << m3::fmt(obj->ident, "#x"));
        obj->srv->send(smsg.sess, &smsg, sizeof(smsg), false);
    }
}

// done in revoke instead of ~Service, because we hold another reference in the exchange_over_sess
// syscall. this is okay, because we only do that for the root capability, which makes it equivalent
// to performing the action in ~Service.
void ServCapability::revoke(bool) {
    if(is_root()) {
        // first, reset the receive buffer: make all slots not-occupied
        if(obj->rgate()->activated()) {
            PEManager::get().pemux(obj->vpe().peid())->config_rcv_ep(
              obj->rgate()->ep, obj->vpe().id(), 0, *obj->rgate());
        }
        // now, abort everything in the sendqueue
        obj->abort();
    }
}

size_t VPECapability::obj_size() const {
    return sizeof(VPE);
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
       << ", addr=#" << obj->addr
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
       << ", addr=" << obj->addr
       << ", size=" << m3::fmt(obj->size, "#0x", sizeof(label_t) * 2)
       << ", perms=#" << m3::fmt(obj->perms, "x")
       << ", eps=";
    obj->print_eps(os);
    os << "]";
}

void MapCapability::printInfo(m3::OStream &os) const {
    os << ": map  [refs=" << obj->refcount()
       << ", virt=#" << m3::fmt(sel() << PAGE_BITS, "x")
       << ", global=#" << obj->global
       << ", pages=" << length()
       << ", attr=#" << m3::fmt(obj->attr, "x") << "]";
}

void ServCapability::printInfo(m3::OStream &os) const {
    os << ": serv [refs=" << obj->refcount()
       << ", name=" << obj->name() << "]";
}

void SessCapability::printInfo(m3::OStream &os) const {
    os << ": sess [refs=" << obj->refcount()
        << ", serv=" << obj->srv->name()
        << ", ident=#" << m3::fmt(obj->ident, "x") << "]";
}

void PECapability::printInfo(m3::OStream &os) const {
    os << ": pe  [refs=" << obj->refcount()
        << ", pe=" << obj->id
        << ", eps=" << obj->eps
        << ", vpes=" << obj->vpes << "]";
}

void EPCapability::printInfo(m3::OStream &os) const {
    os << ": ep  [refs=" << obj->refcount()
        << ", pe=" << obj->pe->id
        << ", ep=" << obj->ep
        << ", replies=" << obj->replies << "]";
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
