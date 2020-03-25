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

#pragma once

#include <base/col/Treap.h>

#include "com/Service.h"
#include "mem/SlabCache.h"
#include "Types.h"

namespace kernel {

class CapTable;
class Capability;
class EPObject;
class GateObject;

m3::OStream &operator<<(m3::OStream &os, const Capability &cc);

class Capability : public m3::TreapNode<Capability, capsel_t> {
    friend class CapTable;

    static const uint CLONE   = 0x8000;

public:
    typedef capsel_t key_t;

    enum {
        SERV    = 0x01,
        SESS    = 0x02,
        SGATE   = 0x04,
        RGATE   = 0x08,
        MGATE   = 0x10,
        MAP     = 0x20,
        VIRTPE  = 0x40,
        PE      = 0x80,
        EP      = 0x100,
        KMEM    = 0x200,
        SEM     = 0x400,
    };

    explicit Capability(CapTable *tbl, capsel_t sel, unsigned type, uint len = 1)
        : TreapNode(sel),
          _type(type),
          _length(len),
          _tbl(tbl),
          _child(),
          _parent(),
          _next(),
          _prev() {
    }
    virtual ~Capability() {
    }

    bool matches(key_t key) {
        return key >= sel() && key < sel() + _length;
    }

    virtual size_t obj_size() const = 0;

    uint type() const {
        return _type & ~static_cast<uint>(CLONE);
    }
    uint length() const {
        return _length;
    }

    bool is_root() const {
        return (_type & CLONE) == 0;
    }

    capsel_t sel() const {
        return key();
    }
    CapTable *table() {
        return _tbl;
    }
    const CapTable *table() const {
        return _tbl;
    }
    Capability *next() {
        return _next;
    }
    const Capability *next() const {
        return _next;
    }
    Capability *parent() {
        return _parent;
    }
    const Capability *parent() const {
        return _parent;
    }
    Capability *child() {
        return _child;
    }
    const Capability *child() const {
        return _child;
    }
    void put(CapTable *tbl, capsel_t sel) {
        _tbl = tbl;
        key(sel);
    }

    void print(m3::OStream &os) const;
    virtual void printInfo(m3::OStream &os) const = 0;
    void printChilds(m3::OStream &os, size_t layer = 0) const;

    virtual GateObject *as_gate() {
        return nullptr;
    }

protected:
    template<class T>
    static T *do_clone(const T *cap, CapTable *tbl, capsel_t sel) {
        auto clone = new T(*cap);
        clone->_type |= CLONE;
        clone->put(tbl, sel);
        return clone;
    }
    void make_clone() {
        _type |= CLONE;
    }

private:
    virtual bool can_revoke() const {
        return true;
    }
    virtual void revoke() {
    }
    virtual Capability *clone(CapTable *tbl, capsel_t sel) const = 0;

    uint _type;
    uint _length;
    CapTable *_tbl;
    Capability *_child;
    Capability *_parent;
    Capability *_next;
    Capability *_prev;
};

class GateObject {
public:
    struct EPUser : public SlabObject<EPUser>, public m3::SListItem {
        explicit EPUser(EPObject *_ep)
            : m3::SListItem(),
              ep(_ep) {
        }
        EPObject *ep;
    };

    explicit GateObject(uint _type)
        : type(_type),
          epuser() {
    }

    EPObject *ep_of_pe(peid_t pe);

    void add_ep(EPObject *ep) {
        epuser.append(new EPUser(ep));
    }
    void remove_ep(EPObject *ep) {
        delete epuser.remove_if([ep](EPUser *u) { return u->ep == ep; });
    }

    void revoke();

    void print_eps(m3::OStream &os) const;

    uint type;
    m3::SList<EPUser> epuser;
};

class RGateObject : public SlabObject<RGateObject>, public GateObject, public m3::RefCounted {
public:
    explicit RGateObject(uint _order, uint _msgorder)
        : GateObject(Capability::RGATE),
          RefCounted(),
          valid(true),
          pe(),
          ep(),
          addr(),
          order(_order),
          msgorder(_msgorder) {
    }

    bool activated() const {
        return addr != 0;
    }
    size_t size() const {
        return 1UL << order;
    }

    bool valid;
    peid_t pe;
    epid_t ep;
    goff_t addr;
    uint order;
    uint msgorder;
};

class SGateObject : public SlabObject<SGateObject>, public GateObject, public m3::RefCounted {
public:
    explicit SGateObject(RGateObject *_rgate, label_t _label, uint _credits)
        : GateObject(Capability::SGATE),
          RefCounted(),
          rgate(_rgate),
          label(_label),
          credits(_credits),
          activated() {
    }

    bool rgate_valid() const {
        return rgate && rgate->valid;
    }

    m3::Reference<RGateObject> rgate;
    label_t label;
    uint credits;
    bool activated;
};

class MGateObject : public SlabObject<MGateObject>, public GateObject, public m3::RefCounted {
public:
    explicit MGateObject(peid_t _pe, vpeid_t _vpe, goff_t _addr, size_t _size, uint _perms)
        : GateObject(Capability::MGATE),
          RefCounted(),
          pe(_pe),
          vpe(_vpe),
          addr(_addr),
          size(_size),
          perms(_perms) {
    }

    peid_t pe;
    vpeid_t vpe;
    goff_t addr;
    size_t size;
    uint perms;
};

class SessObject : public SlabObject<SessObject>, public m3::RefCounted {
public:
    explicit SessObject(Service *_srv, word_t _ident)
        : RefCounted(),
          ident(_ident),
          srv(_srv) {
    }

    void drop_msgs();

    word_t ident;
    m3::Reference<Service> srv;
};

class PEObject : public SlabObject<PEObject>, public m3::RefCounted {
public:
    explicit PEObject(peid_t _id, uint _eps)
        : RefCounted(),
          id(_id),
          eps(_eps),
          vpes() {
    }

    bool has_quota(uint eps) const {
        return this->eps >= eps;
    }
    void alloc(uint eps);
    void free(uint eps);

    peid_t id;
    uint eps;
    uint vpes;
};

class EPObject : public SlabObject<EPObject>, public m3::RefCounted, public m3::DListItem {
public:
    explicit EPObject(PEObject *_pe, bool _is_std, VPE *_vpe, epid_t _ep, uint _replies);
    ~EPObject();

    bool is_std;
    VPE *vpe;
    epid_t ep;
    uint replies;
    m3::Reference<PEObject> pe;
    GateObject *gate;
};

class MapObject : public SlabObject<MapObject>, public m3::RefCounted {
public:
    explicit MapObject(gaddr_t _phys, uint _attr)
        : RefCounted(),
          phys(_phys),
          attr(_attr) {
    }

    gaddr_t phys;
    uint attr;
};

class KMemObject : public SlabObject<KMemObject>, public m3::RefCounted {
public:
    explicit KMemObject(size_t _quota);
    ~KMemObject();

    bool has_quota(size_t size) const {
        return left >= size;
    }
    bool alloc(VPE &vpe, size_t size);
    void free(VPE &vpe, size_t size);

    size_t quota;
    size_t left;
};

class SemObject : public SlabObject<SemObject>, public m3::RefCounted {
public:
    explicit SemObject(uint _counter)
        : SlabObject<SemObject>(),
          m3::RefCounted(),
          counter(_counter),
          waiters(0) {
    }
    ~SemObject();

    m3::Errors::Code down();
    void up();

    uint counter;
    int waiters;
};

class RGateCapability : public SlabObject<RGateCapability>, public Capability {
public:
    explicit RGateCapability(CapTable *tbl, capsel_t sel, RGateObject *_obj)
        : Capability(tbl, sel, RGATE),
          obj(_obj) {
    }

    virtual size_t obj_size() const override {
        return sizeof(RGateObject);
    }
    virtual GateObject *as_gate() override {
        return &*obj;
    }

    void printInfo(m3::OStream &os) const override;

protected:
    virtual void revoke() override;
    virtual Capability *clone(CapTable *tbl, capsel_t sel) const override {
        return do_clone(this, tbl, sel);
    }

public:
    m3::Reference<RGateObject> obj;
};

class SGateCapability : public SlabObject<SGateCapability>, public Capability {
public:
    explicit SGateCapability(CapTable *tbl, capsel_t sel, SGateObject *_obj)
        : Capability(tbl, sel, SGATE),
          obj(_obj) {
    }

    virtual size_t obj_size() const override {
        return sizeof(SGateObject);
    }
    virtual GateObject *as_gate() override {
        return &*obj;
    }

    void printInfo(m3::OStream &os) const override;

protected:
    virtual void revoke() override {
        if(is_root())
            obj->revoke();
    }
    virtual Capability *clone(CapTable *tbl, capsel_t sel) const override {
        return do_clone(this, tbl, sel);
    }

public:
    m3::Reference<SGateObject> obj;
};

class MGateCapability : public SlabObject<MGateCapability>, public Capability {
public:
    explicit MGateCapability(CapTable *tbl, capsel_t sel, MGateObject *_obj)
        : Capability(tbl, sel, MGATE),
          obj(_obj) {
    }

    virtual size_t obj_size() const override {
        return sizeof(MGateObject);
    }
    virtual GateObject *as_gate() override {
        return &*obj;
    }

    void printInfo(m3::OStream &os) const override;

private:
    virtual void revoke() override {
        if(is_root())
            obj->revoke();
    }
    virtual Capability *clone(CapTable *tbl, capsel_t sel) const override {
        return do_clone(this, tbl, sel);
    }

public:
    m3::Reference<MGateObject> obj;
};

class MapCapability : public SlabObject<MapCapability>, public Capability {
public:
    enum : uint {
        EXCL    = 0x08000,
        KERNEL  = 0x10000,
    };

    explicit MapCapability(CapTable *tbl, capsel_t sel, uint _pages, MapObject *_obj)
        : Capability(tbl, sel, MAP, _pages),
          obj(_obj) {
    }

    virtual size_t obj_size() const override {
        return sizeof(MapObject);
    }

    m3::Errors::Code remap(gaddr_t _phys, uint _attr);

    void printInfo(m3::OStream &os) const override;

private:
    virtual bool can_revoke() const override {
        return (obj->attr & KERNEL) == 0;
    }
    virtual void revoke() override;
    virtual Capability *clone(CapTable *, capsel_t) const override {
        // not clonable
        return nullptr;
    }

public:
    m3::Reference<MapObject> obj;
};

class ServCapability : public SlabObject<ServCapability>, public Capability {
public:
    explicit ServCapability(CapTable *tbl, capsel_t sel, Service *_obj)
        : Capability(tbl, sel, SERV),
          obj(_obj) {
    }

    virtual size_t obj_size() const override {
        return sizeof(Service);
    }

    void printInfo(m3::OStream &os) const override;

private:
    virtual void revoke() override;
    virtual Capability *clone(CapTable *tbl, capsel_t sel) const override {
        return do_clone(this, tbl, sel);
    }

public:
    m3::Reference<Service> obj;
};

class SessCapability : public SlabObject<SessCapability>, public Capability {
public:
    explicit SessCapability(CapTable *tbl, capsel_t sel, SessObject *_obj)
        : Capability(tbl, sel, SESS),
          obj(_obj) {
    }

    virtual size_t obj_size() const override {
        return sizeof(SessObject);
    }

    void printInfo(m3::OStream &os) const override;

private:
    virtual void revoke() override;
    virtual Capability *clone(CapTable *tbl, capsel_t sel) const override {
        return do_clone(this, tbl, sel);
    }

public:
    m3::Reference<SessObject> obj;
};

class PECapability : public SlabObject<PECapability>, public Capability {
    friend class VPE;
public:
    explicit PECapability(CapTable *tbl, capsel_t sel, PEObject *_obj)
        : Capability(tbl, sel, PE),
          obj(_obj) {
    }

    virtual size_t obj_size() const override {
        return sizeof(PEObject);
    }

    void printInfo(m3::OStream &os) const override;

private:
    virtual bool can_revoke() const override {
        // revoking with VPEs is considered a violation of the API.
        return obj->vpes == 0;
    }
    virtual void revoke() override;
    virtual Capability *clone(CapTable *tbl, capsel_t sel) const override {
        return do_clone(this, tbl, sel);
    }

public:
    m3::Reference<PEObject> obj;
};

class EPCapability : public SlabObject<EPCapability>, public Capability {
public:
    explicit EPCapability(CapTable *tbl, capsel_t sel, EPObject *_obj)
        : Capability(tbl, sel, EP),
          obj(_obj) {
    }

    virtual size_t obj_size() const override {
        return sizeof(EPObject);
    }

    void printInfo(m3::OStream &os) const override;

private:
    virtual Capability *clone(CapTable *tbl, capsel_t sel) const override {
        return do_clone(this, tbl, sel);
    }

public:
    m3::Reference<EPObject> obj;
};

class VPECapability : public SlabObject<VPECapability>, public Capability {
public:
    explicit VPECapability(CapTable *tbl, capsel_t sel, VPE *_obj)
        : Capability(tbl, sel, VIRTPE),
          obj(_obj) {
    }

    virtual size_t obj_size() const override;

    void printInfo(m3::OStream &os) const override;

private:
    virtual Capability *clone(CapTable *tbl, capsel_t sel) const override {
        return do_clone(this, tbl, sel);
    }

public:
    m3::Reference<VPE> obj;
};

class KMemCapability : public SlabObject<KMemCapability>, public Capability {
    friend class VPE;
public:
    explicit KMemCapability(CapTable *tbl, capsel_t sel, KMemObject *_obj)
        : Capability(tbl, sel, KMEM),
          obj(_obj) {
    }

    virtual size_t obj_size() const override {
        return sizeof(KMemObject);
    }

    void printInfo(m3::OStream &os) const override;

private:
    virtual bool can_revoke() const override {
        // revoking with non-full quota is considered a violation of the API. this can only happen
        // if there are still VPEs using this quota, in which case it shouldn't be revoked
        return obj->left == obj->quota;
    }
    virtual void revoke() override;
    virtual Capability *clone(CapTable *tbl, capsel_t sel) const override {
        return do_clone(this, tbl, sel);
    }

public:
    m3::Reference<KMemObject> obj;
};

class SemCapability : public SlabObject<SemCapability>, public Capability {
public:
    explicit SemCapability(CapTable *tbl, capsel_t sel, SemObject *_obj)
        : Capability(tbl, sel, SEM),
          obj(_obj) {
    }

    virtual size_t obj_size() const override {
        return sizeof(SemObject);
    }

    void printInfo(m3::OStream &os) const override;

private:
    virtual Capability *clone(CapTable *tbl, capsel_t sel) const override {
        return do_clone(this, tbl, sel);
    }

public:
    m3::Reference<SemObject> obj;
};

inline EPObject *GateObject::ep_of_pe(peid_t pe) {
    for(auto u = epuser.begin(); u != epuser.end(); ++u) {
        if(u->ep->pe->id == pe)
            return u->ep;
    }
    return nullptr;
}

inline void GateObject::print_eps(m3::OStream &os) const {
    os << "[";
    for(auto u = epuser.begin(); u != epuser.end(); ) {
        os << "PE" << u->ep->pe->id
           << ":EP" << u->ep->ep << "(" << u->ep->replies << " replies)";
        if(++u != epuser.end())
            os << ", ";
    }
    os << "]";
}

}
