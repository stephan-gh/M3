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
#include "Types.h"

namespace kernel {

class PEManager;
class VPECapability;
class VPEManager;

#define CREATE_CAP(CAP, KOBJ, tbl, sel, ...)                               \
    (tbl)->vpe().kmem()->alloc((tbl)->vpe(), sizeof(CAP) + sizeof(KOBJ)) ? \
        new CAP(tbl, sel, new KOBJ(__VA_ARGS__))                         : \
        nullptr

class VPE : public m3::SListItem, public SlabObject<VPE>, public m3::RefCounted {
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
    static const epid_t INVALID_EP      = static_cast<epid_t>(-1);

    static const int SYSC_MSGSIZE_ORD   = m3::nextlog2<512>::val;
    static const int SYSC_CREDIT_ORD    = SYSC_MSGSIZE_ORD;
    static const int NOTIFY_MSGSIZE_ORD = m3::nextlog2<64>::val;

    static size_t base_kmem() {
        // the child pays for the VPE, because it owns the root cap, i.e., free's the memory later
        return sizeof(VPE) + sizeof(AddrSpace) +
               // VPE cap and memory cap
               sizeof(VPECapability) + sizeof(MGateCapability) + sizeof(MGateObject) +
               // EP caps
               (EP_COUNT - m3::DTU::FIRST_FREE_EP) * (sizeof(EPCapability) + sizeof(EPObject));
    }
    static size_t extra_kmem(const m3::PEDesc &pe) {
        // for VM PEs we need the root PT
        // additionally, we need space for PEMux, its page tables etc.
        return (pe.has_virtmem() ? PAGE_SIZE : 0u) + VPE_EXTRA_MEM;
    }

    enum State {
        RUNNING,
        DEAD
    };

    enum Flags {
        F_BOOTMOD     = 1 << 0,
        F_HASAPP      = 1 << 1,
        F_STOPPED     = 1 << 2,
    };

    explicit VPE(m3::String &&prog, peid_t peid, vpeid_t id, uint flags, KMemObject *kmem,
                 epid_t sep = INVALID_EP, epid_t rep = INVALID_EP, capsel_t sgate = m3::KIF::INV_SEL);
    VPE(const VPE &) = delete;
    VPE &operator=(const VPE &) = delete;
    ~VPE();

    vpeid_t id() const {
        return desc().id;
    }
    const m3::String &name() const {
        return _name;
    }
    const m3::Reference<KMemObject> &kmem() {
        return _kmem;
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

    bool has_app() const {
        return _flags & F_HASAPP;
    }
    bool is_stopped() const {
        return _flags & F_STOPPED;
    }
    bool is_on_pe() const {
        return state() == RUNNING;
    }
    State state() const {
        return _state;
    }

    AddrSpace *address_space() {
        return _as;
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

    CapTable &objcaps() {
        return _objcaps;
    }
    CapTable &mapcaps() {
        return _mapcaps;
    }

    SendQueue &upcall_queue() {
        return _upcqueue;
    }

    void upcall(const void *msg, size_t size, bool onheap) {
        _upcqueue.send(m3::DTU::UPCALL_REP, 0, msg, size, onheap);
    }
    void upcall_vpewait(word_t event, m3::KIF::Syscall::VPEWaitReply &reply);

    void start_app(int pid);
    void stop_app(int exitcode, bool self);
    void wakeup();

    bool check_exits(const xfer_t *sels, size_t count, m3::KIF::Syscall::VPEWaitReply &reply);
    void wait_exit_async(xfer_t *sels, size_t count, m3::KIF::Syscall::VPEWaitReply &reply);

    bool invalidate_ep(epid_t ep, bool force = false);

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

    VPEDesc _desc;
    uint _flags;
    int _pid;
    State _state;
    int _exitcode;
    epid_t _sysc_ep;
    m3::Reference<KMemObject> _kmem;
    m3::String _name;
    CapTable _objcaps;
    CapTable _mapcaps;
    size_t _rbufs_size;
    SendQueue _upcqueue;
    volatile xfer_t *_vpe_wait_sels;
    volatile size_t _vpe_wait_count;
    AddrSpace *_as;
    capsel_t _first_sel;
    goff_t _mem_base;
};

}
