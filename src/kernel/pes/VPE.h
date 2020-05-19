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

#include <base/KIF.h>

#include "cap/CapTable.h"
#include "pes/VPEDesc.h"
#include "Types.h"

namespace kernel {

class PEMux;
class PEManager;
class VPECapability;
class VPEManager;

#define CREATE_CAP(CAP, KOBJ, tbl, sel, ...) \
    CREATE_CAP_SIZE(CAP, KOBJ, sizeof(CAP) + sizeof(KOBJ), tbl, sel, ##__VA_ARGS__)

#define CREATE_CAP_SIZE(CAP, KOBJ, size, tbl, sel, ...) \
    (tbl)->vpe()->kmem()->alloc(*(tbl)->vpe(), size) ?  \
        new CAP(tbl, sel, new KOBJ(__VA_ARGS__))     :  \
        nullptr

class VPE : public SlabObject<VPE>, public m3::RefCounted {
    friend class PEMux;
    friend class PEManager;
    friend class VPECapability;
    friend class VPEManager;

    struct ServName : public m3::SListItem {
        explicit ServName(const m3::String &_name) : name(_name) {
        }
        m3::String name;
    };

public:
    static const uint16_t INVALID_ID    = 0xFFFF;
    static const uint16_t KERNEL_ID     = INVALID_ID;
    static const epid_t INVALID_EP      = static_cast<epid_t>(-1);

    static const int SYSC_MSGSIZE_ORD   = m3::nextlog2<512>::val;
    static const int SYSC_CREDIT_ORD    = SYSC_MSGSIZE_ORD;

    static size_t required_kmem() {
        // the child pays for the VPE, because it owns the root cap, i.e., free's the memory later
        return sizeof(VPE) +
               // PE cap, VPE cap, and kmem cap
               sizeof(PECapability) + sizeof(VPECapability) + sizeof(KMemCapability);
    }

    enum State {
        RUNNING,
        DEAD
    };

    enum Flags {
        F_ROOT        = 1 << 0,
        F_HASAPP      = 1 << 1,
        F_STOPPED     = 1 << 2,
    };

    explicit VPE(m3::String &&prog, PECapability *pecap, epid_t eps_start, vpeid_t id,
                 uint flags, KMemCapability *kmemcap);
    VPE(const VPE &) = delete;
    VPE &operator=(const VPE &) = delete;
    ~VPE();

    vpeid_t id() const {
        return desc().id;
    }
    const m3::String &name() const {
        return _name;
    }
    const m3::Reference<KMemObject> &kmem() const {
        return _kmem;
    }
    const m3::Reference<PEObject> &pe() const {
        return _pe;
    }

    const VPEDesc &desc() const {
        return _desc;
    }
    peid_t peid() const {
        return desc().pe;
    }

    m3::GlobAddr rbuf_phys() const {
        return _rbuf_phys;
    }
    epid_t eps_start() const {
        return _eps_start;
    }
    epid_t syscall_ep() const {
        return _sysc_ep;
    }

    int pid() const {
        return _pid;
    }

    bool has_app() const {
        return _flags & F_HASAPP;
    }
    bool is_stopped() const {
        return _flags & F_STOPPED;
    }
    bool is_running() const {
        return _state == RUNNING;
    }

    void set_mem_base(goff_t addr);

    int exitcode() const {
        return _exitcode;
    }
    static void wait_for_exit();

    CapTable &objcaps() {
        return _objcaps;
    }
    CapTable &mapcaps() {
        return _mapcaps;
    }

    void add_ep(EPObject *ep) {
        _eps.append(ep);
    }
    void remove_ep(EPObject *ep) {
        _eps.remove(ep);
    }

    void set_pg_sep(EPObject *ep) {
        _pg_sep = ep;
    }
    void set_pg_rep(EPObject *ep) {
        _pg_rep = ep;
    }

    void upcall(const void *msg, size_t size, bool onheap) {
        _upcqueue.send(0, msg, size, onheap);
    }
    void upcall_vpewait(word_t event, m3::KIF::Syscall::VPEWaitReply &reply);

    void start_app(int pid);
    void stop_app(int exitcode, bool self);

    bool check_exits(const xfer_t *sels, size_t count, m3::KIF::Syscall::VPEWaitReply &reply);
    void wait_exit_async(xfer_t *sels, size_t count, m3::KIF::Syscall::VPEWaitReply &reply);

private:
    void set_first_sel(capsel_t sel) {
        _first_sel = sel;
    }

    void init_eps();
    void init_memory();
    void load_root(m3::GlobAddr env_phys);
    void exit_app(int exitcode);

    VPEDesc _desc;
    uint _flags;
    int _pid;
    State _state;
    int _exitcode;
    epid_t _sysc_ep;
    epid_t _eps_start;
    m3::GlobAddr _rbuf_phys;
    m3::Reference<KMemObject> _kmem;
    m3::Reference<PEObject> _pe;
    m3::DList<EPObject> _eps;
    EPObject *_pg_sep;
    EPObject *_pg_rep;
    m3::String _name;
    CapTable _objcaps;
    CapTable _mapcaps;
    SendQueue _upcqueue;
    volatile xfer_t *_vpe_wait_sels;
    volatile size_t _vpe_wait_count;
    capsel_t _first_sel;
};

}
