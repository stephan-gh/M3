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

#include "cap/CapTable.h"
#include "pes/VPE.h"
#include "pes/VPEManager.h"

namespace kernel {

VPE *CapTable::vpe() const {
    if(_vpe != VPE::INVALID_ID)
        return &VPEManager::get().vpe(_vpe);
    return nullptr;
}

void CapTable::revoke_all(bool remove_vpe) {
    m3::Treap<Capability> tmp;

    // TODO it might be better to do that in a different order, because it is more expensive to
    // remove a node that has two childs (it requires a rotate). Thus, it would be better to start
    // with leaf nodes.
    Capability *c;
    while((c = _caps.remove_root()) != nullptr) {
        if(remove_vpe || c->sel() >= m3::KIF::FIRST_FREE_SEL) {
            revoke(c, false);
            // hack for self-referencing VPE capability. we can't dereference it here, because if we
            // force-destruct a VPE, there might be other references, so that it breaks if we decrease
            // the counter (the self-reference did not increase it).
            if(c->sel() == m3::KIF::SEL_VPE)
                static_cast<VPECapability*>(c)->obj.forget();
            delete c;
        }
        // put the caps we don't want to delete now in a temporary treap
        else
            tmp.insert(c);
    }

    // insert them again
    while((c = tmp.remove_root()) != nullptr)
        _caps.insert(c);
}

Capability *CapTable::obtain(capsel_t dst, Capability *c) {
    static_assert(sizeof(SGateCapability) == sizeof(RGateCapability) &&
                  sizeof(SGateCapability) == sizeof(MGateCapability) &&
                  sizeof(SGateCapability) == sizeof(MapCapability) &&
                  sizeof(SGateCapability) == sizeof(ServCapability) &&
                  sizeof(SGateCapability) == sizeof(EPCapability) &&
                  sizeof(SGateCapability) == sizeof(PECapability) &&
                  sizeof(SGateCapability) == sizeof(VPECapability) &&
                  sizeof(SGateCapability) == sizeof(KMemCapability), "Cap sizes not equal");

    Capability *nc = c;
    if(c) {
        VPE *v = vpe();
        if(v && !v->kmem()->alloc(*v, sizeof(SGateCapability)))
            return nullptr;

        nc = c->clone(this, dst);
        if(nc)
            inherit(c, nc);
    }
    set(dst, nc);
    return nc;
}

void CapTable::inherit(Capability *parent, Capability *child) {
    child->_parent = parent;
    child->_child = nullptr;
    child->_next = parent->_child;
    child->_prev = nullptr;
    if(child->_next)
        child->_next->_prev = child;
    parent->_child = child;
}

void CapTable::revoke_rec(Capability *c, bool revnext) {
    Capability *child = c->child();
    Capability *next = c->next();

    auto *vpe = c->table()->vpe();
    if(vpe) {
        vpe->kmem()->free(*vpe, sizeof(SGateCapability));
        if(c->is_root())
            vpe->kmem()->free(*vpe, c->obj_size());
    }

    // set that before we descent to childs and siblings
    c->_type |= Capability::IN_REVOCATION;

    if(child)
        revoke_rec(child, true);
    // on the first level, we don't want to revoke siblings
    if(revnext && next)
        revoke_rec(next, true);

    // delete the object here to allow the child capabilities to use their parent pointer
    bool exists = c->table()->unset(c->sel());
    // and we want to give caps a chance to perform some actions after making the cap inaccessible
    c->revoke();
    if(exists)
        delete c;
}

void CapTable::revoke(Capability *c, bool revnext) {
    if(c->_next)
        c->_next->_prev = c->_prev;
    if(c->_prev)
        c->_prev->_next = c->_next;
    if(c->_parent && c->_parent->_child == c)
        c->_parent->_child = revnext ? nullptr : c->_next;
    revoke_rec(c, revnext);
}

m3::Errors::Code CapTable::revoke(const m3::KIF::CapRngDesc &crd, bool own) {
    m3::Errors::Code res = m3::Errors::NONE;
    for(capsel_t i = crd.start(), end = crd.start() + crd.count(); i < end; ) {
        Capability *c = get(i);
        i = c ? c->sel() + c->length() : i + 1;
        if(c) {
            if(!c->can_revoke()) {
                KLOG(INFO, "Warning: trying to revoke unrevocable cap: " << *c);
                res = m3::Errors::NOT_REVOCABLE;
                continue;
            }

            if(own)
                revoke(c, false);
            else
                revoke(c->_child, true);
        }
    }
    return res;
}

m3::OStream &operator<<(m3::OStream &os, const CapTable &ct) {
    os << "CapTable[VPE" << ct._vpe << "]:\n";
    ct._caps.print(os, false);
    return os;
}

}
