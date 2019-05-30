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

#include <base/col/SList.h>
#include <base/KIF.h>

#include <thread/ThreadManager.h>

#include "cap/CapTable.h"
#include "mem/AddrSpace.h"
#include "mem/SlabCache.h"
#include "pes/VPEDesc.h"
#include "pes/VPEGroup.h"
#include "DTUState.h"
#include "Types.h"

namespace kernel {

class ContextSwitcher;
class PEManager;
class VPECapability;
class VPEGroup;
class VPEManager;

class VPE : public m3::SListItem, public SlabObject<VPE>, public m3::RefCounted {
    friend class ContextSwitcher;
    friend class PEManager;
    friend class VPECapability;
    friend class VPEGroup;
    friend class VPEManager;

    struct ServName : public m3::SListItem {
        explicit ServName(const m3::String &_name) : name(_name) {
        }
        m3::String name;
    };

public:
    static const uint16_t INVALID_ID    = 0xFFFF;
    static const epid_t INVALID_EP      = static_cast<epid_t>(-1);

    static const cycles_t APP_YIELD     = 20000;
    static const cycles_t SRV_YIELD     = 1;

    static const int SYSC_MSGSIZE_ORD   = m3::nextlog2<512>::val;
    static const int SYSC_CREDIT_ORD    = SYSC_MSGSIZE_ORD;
    static const int NOTIFY_MSGSIZE_ORD = m3::nextlog2<64>::val;

    enum State {
        RUNNING,
        SUSPENDED,
        RESUMING,
        DEAD
    };

    enum Flags {
        F_BOOTMOD     = 1 << 0,
        F_IDLE        = 1 << 1,
        F_INIT        = 1 << 2,
        F_HASAPP      = 1 << 3,
        F_MUXABLE     = 1 << 4, // TODO temporary
        F_READY       = 1 << 5,
        F_WAITING     = 1 << 6,
        F_NEEDS_INVAL = 1 << 7,
        F_FLUSHED     = 1 << 8,
        F_NOBLOCK     = 1 << 9,
        F_PINNED      = 1 << 10,
        F_YIELDED     = 1 << 11,
    };

    explicit VPE(m3::String &&prog, peid_t peid, vpeid_t id, uint flags, epid_t sep = INVALID_EP,
                 epid_t rep = INVALID_EP, capsel_t sgate = m3::KIF::INV_SEL, VPEGroup *group = nullptr);
    VPE(const VPE &) = delete;
    VPE &operator=(const VPE &) = delete;
    ~VPE();

    vpeid_t id() const {
        return desc().id;
    }
    const m3::String &name() const {
        return _name;
    }
    const m3::Reference<VPEGroup> &group() const {
        return _group;
    }

    const VPEDesc &desc() const {
        return _desc;
    }
    peid_t pe() const {
        return desc().pe;
    }
    void set_pe(peid_t pe) {
        _desc.pe = pe;
    }

    epid_t syscall_ep() const {
        return _sysc_ep;
    }

    int pid() const {
        return _pid;
    }

    bool has_yielded() const {
        return _flags & F_YIELDED;
    }
    bool is_idle() const {
        return _flags & F_IDLE;
    }
    bool has_app() const {
        return _flags & F_HASAPP;
    }
    bool is_on_pe() const {
        return state() == RUNNING || state() == RESUMING;
    }
    bool is_pinned() const {
        return _flags & F_PINNED;
    }
    State state() const {
        return _state;
    }

    AddrSpace *address_space() {
        return _as;
    }
    const MainMemory::Allocation &recvbuf_copy() const {
        return _rbufcpy;
    }

    goff_t mem_base() const {
        return _mem_base;
    }
    goff_t eps_base() const {
        return mem_base();
    }
    goff_t rbuf_base() const {
        return mem_base() + EPMEM_SIZE;
    }
    void set_mem_base(goff_t addr) {
        _mem_base = addr;
        finish_start();
    }

    int exitcode() const {
        return _exitcode;
    }
    static void wait_for_exit();

    bool is_waiting() const {
        return _flags & F_WAITING;
    }
    void start_wait() {
        assert(!(_flags & F_WAITING));
        _flags |= F_WAITING;
    }
    void stop_wait() {
        assert(_flags & F_WAITING);
        _flags ^= F_WAITING;
    }

    CapTable &objcaps() {
        return _objcaps;
    }
    CapTable &mapcaps() {
        return _mapcaps;
    }

    SendGate &upcall_sgate() {
        return _upcsgate;
    }

    SendQueue &upcall_queue() {
        return _upcqueue;
    }

    void upcall(const void *msg, size_t size, bool onheap) {
        _upcqueue.send(&_upcsgate, 0, msg, size, onheap);
    }
    void upcall_forward(word_t event, m3::Errors::Code res);
    void upcall_vpewait(word_t event, m3::KIF::Syscall::VPEWaitReply &reply);

    void add_forward() {
        _pending_fwds++;
    }
    void rem_forward() {
        _pending_fwds--;
    }

    cycles_t yield_time() const {
        if(_group)
            return APP_YIELD;
        return _services > 0 || !Platform::pe(pe()).is_programmable() ? SRV_YIELD : APP_YIELD;
    }
    void add_service() {
        _services++;
    }
    void rem_service() {
        _services--;
    }

    void needs_invalidate() {
        _flags |= F_NEEDS_INVAL;
    }
    void flush_cache();

    void start_app(int pid);
    void stop_app(int exitcode, bool self);

    void yield();

    bool migrate(bool fast);
    bool migrate_for(VPE *vpe);
    bool resume(bool need_app = true, bool unblock = true);
    void wakeup();

    bool invalidate_ep(epid_t ep, bool force = false);

    bool can_forward_msg(epid_t ep);
    void forward_msg(epid_t ep, peid_t pe, vpeid_t vpe);
    void forward_mem(epid_t ep, peid_t pe);

    m3::Errors::Code config_rcv_ep(epid_t ep, RGateObject &obj);
    m3::Errors::Code config_snd_ep(epid_t ep, SGateObject &obj);
    m3::Errors::Code config_mem_ep(epid_t ep, const MGateObject &obj, goff_t off = 0);

    void set_first_sel(capsel_t sel) {
        _first_sel = sel;
    }

private:
    void finish_start();
    void init_eps();
    void init_memory();
    void load_app();
    void exit_app(int exitcode);

    void update_ep(epid_t ep);

    void notify_resume();

    VPEDesc _desc;
    uint _flags;
    int _pid;
    State _state;
    int _exitcode;
    epid_t _sysc_ep;
    m3::Reference<VPEGroup> _group;
    uint _services;
    uint _pending_fwds;
    m3::String _name;
    CapTable _objcaps;
    CapTable _mapcaps;
    uint64_t _lastsched;
    size_t _rbufs_size;
    alignas(DTU_PKG_SIZE) DTUState _dtustate;
    SendGate _upcsgate;
    SendQueue _upcqueue;
    AddrSpace *_as;
    size_t _headers;
    MainMemory::Allocation _rbufcpy;
    capsel_t _first_sel;
    goff_t _mem_base;
};

}
