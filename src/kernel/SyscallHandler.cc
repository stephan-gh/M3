/*
 * Copyright (C) 2015-2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/tracing/Tracing.h>
#include <base/log/Kernel.h>
#include <base/util/Math.h>
#include <base/Init.h>
#include <base/Panic.h>

#include <thread/ThreadManager.h>

#include "com/Services.h"
#include "pes/PEManager.h"
#include "pes/VPEManager.h"
#include "DTU.h"
#include "Platform.h"
#include "SyscallHandler.h"
#include "WorkLoop.h"

namespace kernel {

ulong SyscallHandler::_vpes_per_ep[SyscallHandler::SYSC_REP_COUNT];
SyscallHandler::handler_func SyscallHandler::_callbacks[m3::KIF::Syscall::COUNT];

#define LOG_SYS(vpe, sysname, expr)                                                         \
        KLOG(SYSC, (vpe)->id() << ":" << (vpe)->name() << "@" << m3::fmt((vpe)->pe(), "X")  \
            << (sysname) << expr)

#define LOG_ERROR(vpe, error, msg)                                                          \
    do {                                                                                    \
        KLOG(ERR, "\e[37;41m"                                                               \
            << (vpe)->id() << ":" << (vpe)->name() << "@" << m3::fmt((vpe)->pe(), "X")      \
            << ": " << msg << " (" << error << ")\e[0m");                                   \
    }                                                                                       \
    while(0)

#define SYS_ERROR(vpe, msg, errcode, errmsg) {                                              \
        LOG_ERROR(vpe, errcode, errmsg);                                                    \
        reply_result((vpe), (msg), (errcode));                                              \
        return;                                                                             \
    }

#define SYS_CREATE_CAP(vpe, msg, CAP, KOBJ, tbl, sel, ...) ({                               \
        auto cap = CREATE_CAP(CAP, KOBJ, tbl, sel, ##__VA_ARGS__);                          \
        if(cap == nullptr)                                                                  \
            SYS_ERROR(vpe, msg, m3::Errors::NO_KMEM, "Out of kernel memory");               \
        cap;                                                                                \
    })

template<typename T>
static const T *get_message(const m3::DTU::Message *msg) {
    return reinterpret_cast<const T*>(msg->data);
}

void SyscallHandler::init() {
#if !defined(__t2__)
    // configure both receive buffers (we need to do that manually in the kernel)
    // TODO we also need to make sure that a VPE's syscall slot isn't in use if we suspend it
    for(size_t i = 0; i < SYSC_REP_COUNT; ++i) {
        int buford = m3::getnextlog2(32) + VPE::SYSC_MSGSIZE_ORD;
        size_t bufsize = static_cast<size_t>(1) << buford;
        DTU::get().recv_msgs(ep(i),reinterpret_cast<uintptr_t>(new uint8_t[bufsize]),
            buford, VPE::SYSC_MSGSIZE_ORD);
    }

    int buford = m3::nextlog2<1024>::val;
    size_t bufsize = static_cast<size_t>(1) << buford;
    DTU::get().recv_msgs(srvep(), reinterpret_cast<uintptr_t>(new uint8_t[bufsize]),
        buford, m3::nextlog2<256>::val);
#endif

    add_operation(m3::KIF::Syscall::PAGEFAULT,      &SyscallHandler::page_fault);
    add_operation(m3::KIF::Syscall::CREATE_SRV,     &SyscallHandler::create_srv);
    add_operation(m3::KIF::Syscall::CREATE_SESS,    &SyscallHandler::create_sess);
    add_operation(m3::KIF::Syscall::CREATE_RGATE,   &SyscallHandler::create_rgate);
    add_operation(m3::KIF::Syscall::CREATE_SGATE,   &SyscallHandler::create_sgate);
    add_operation(m3::KIF::Syscall::CREATE_VPEGRP,  &SyscallHandler::create_vgroup);
    add_operation(m3::KIF::Syscall::CREATE_VPE,     &SyscallHandler::create_vpe);
    add_operation(m3::KIF::Syscall::CREATE_MAP,     &SyscallHandler::create_map);
    add_operation(m3::KIF::Syscall::ACTIVATE,       &SyscallHandler::activate);
    add_operation(m3::KIF::Syscall::VPE_CTRL,       &SyscallHandler::vpe_ctrl);
    add_operation(m3::KIF::Syscall::VPE_WAIT,       &SyscallHandler::vpe_wait);
    add_operation(m3::KIF::Syscall::DERIVE_MEM,     &SyscallHandler::derive_mem);
    add_operation(m3::KIF::Syscall::DERIVE_KMEM,    &SyscallHandler::derive_kmem);
    add_operation(m3::KIF::Syscall::KMEM_QUOTA,     &SyscallHandler::kmem_quota);
    add_operation(m3::KIF::Syscall::EXCHANGE,       &SyscallHandler::exchange);
    add_operation(m3::KIF::Syscall::DELEGATE,       &SyscallHandler::delegate);
    add_operation(m3::KIF::Syscall::OBTAIN,         &SyscallHandler::obtain);
    add_operation(m3::KIF::Syscall::REVOKE,         &SyscallHandler::revoke);
    add_operation(m3::KIF::Syscall::FORWARD_MSG,    &SyscallHandler::forward_msg);
    add_operation(m3::KIF::Syscall::FORWARD_MEM,    &SyscallHandler::forward_mem);
    add_operation(m3::KIF::Syscall::FORWARD_REPLY,  &SyscallHandler::forward_reply);
    add_operation(m3::KIF::Syscall::NOOP,           &SyscallHandler::noop);
}

void SyscallHandler::reply_msg(VPE *vpe, const m3::DTU::Message *msg, const void *reply, size_t size) {
    while(vpe->state() != VPE::RUNNING) {
        if(!vpe->resume(false))
            return;
    }

    epid_t ep = vpe->syscall_ep();
    DTU::get().reply(ep, reply, size, m3::DTU::get().get_msgoff(ep, msg));
}

void SyscallHandler::reply_result(VPE *vpe, const m3::DTU::Message *msg, m3::Errors::Code code) {
    m3::KIF::DefaultReply reply;
    reply.error = static_cast<xfer_t>(code);
    return reply_msg(vpe, msg, &reply, sizeof(reply));
}

void SyscallHandler::handle_message(VPE *vpe, const m3::DTU::Message *msg) {
    auto req = get_message<m3::KIF::DefaultRequest>(msg);
    m3::KIF::Syscall::Operation op = static_cast<m3::KIF::Syscall::Operation>(req->opcode);

    if(static_cast<size_t>(op) < sizeof(_callbacks) / sizeof(_callbacks[0]))
        _callbacks[op](vpe, msg);
    else
        reply_result(vpe, msg, m3::Errors::INV_ARGS);
}

void SyscallHandler::page_fault(VPE *vpe, const m3::DTU::Message *msg) {
    EVENT_TRACER_Syscall_pagefault();

    m3::Errors::Code res = m3::Errors::NOT_SUP;
#if defined(__gem5__)
    auto req = get_message<m3::KIF::Syscall::Pagefault>(msg);
    uint64_t virt = req->virt;
    uint access = req->access;

    LOG_SYS(vpe, ": syscall::page_fault", "(virt=" << m3::fmt(virt, "p")
        << ", access " << m3::fmt(access, "#x") << ")");

    AddrSpace *as = vpe->address_space();
    if(!as)
        SYS_ERROR(vpe, msg, m3::Errors::NOT_SUP, "No address space / PF handler");

    // get sgate
    auto sgatecap = static_cast<SGateCapability*>(vpe->objcaps().get(as->sgate(), Capability::SGATE));
    if(sgatecap == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid msg cap");

    // get and check EP
    epid_t sep = as->sep();
    if(!vpe->can_forward_msg(sep))
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Send did not fail");

    reply_result(vpe, msg, m3::Errors::NONE);

    // wait for pager
    VPE &tvpe = VPEManager::get().vpe(sgatecap->obj->rgate->vpe);
    res = wait_for(": syscall::page_fault", tvpe, vpe, true);

    if(res == m3::Errors::NONE) {
        // re-enable the EP first, because the reply to the sent message below might otherwise
        // pass credits back BEFORE we overwrote the EP
        vpe->forward_msg(sep, tvpe.pe(), tvpe.id());

        // forward PF msg to pager
        m3::KIF::Syscall::Pagefault pfmsg;
        pfmsg.virt = virt;
        pfmsg.access = access;

        epid_t rep = as->rep();
        uint64_t sender = vpe->pe() | (vpe->id() << 8) | (sep << 24) | (static_cast<uint64_t>(rep) << 32);
        res = DTU::get().send_to(tvpe.desc(), sgatecap->obj->rgate->ep, sgatecap->obj->label,
                                 &pfmsg, sizeof(pfmsg), 0, rep, sender);
    }
    if(res != m3::Errors::NONE)
        LOG_ERROR(vpe, res, "page_fault failed");
#else
    reply_result(vpe, msg, res);
#endif
}

void SyscallHandler::create_srv(VPE *vpe, const m3::DTU::Message *msg) {
    EVENT_TRACER_Syscall_createsrv();

    auto req = get_message<m3::KIF::Syscall::CreateSrv>(msg);
    capsel_t dst = req->dst_sel;
    capsel_t tvpe = req->vpe_sel;
    capsel_t rgate = req->rgate_sel;
    m3::String name(req->name, m3::Math::min(static_cast<size_t>(req->namelen), sizeof(req->name)));

    LOG_SYS(vpe, ": syscall::create_srv", "(dst=" << dst << ", vpe=" << tvpe
        << ", rgate=" << rgate << ", name=" << name << ")");

    if(!vpe->objcaps().unused(dst))
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid server selector");

    auto rgatecap = static_cast<RGateCapability*>(vpe->objcaps().get(rgate, Capability::RGATE));
    if(rgatecap == nullptr || !rgatecap->obj->activated())
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "RGate capability invalid");

    auto vpecap = static_cast<VPECapability*>(vpe->objcaps().get(tvpe, Capability::VIRTPE));
    if(vpecap == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "VPE capability invalid");

    if(name.length() == 0)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid server name");

    auto servcap = SYS_CREATE_CAP(vpe, msg, ServCapability, Service,
        &vpe->objcaps(), dst, *vpecap->obj, name, rgatecap->obj);
    ServiceList::get().add(&*servcap->obj);
    vpe->objcaps().set(dst, servcap);

    reply_result(vpe, msg, m3::Errors::NONE);
}

void SyscallHandler::create_sess(VPE *vpe, const m3::DTU::Message *msg) {
    EVENT_TRACER_Syscall_createsessat();

    auto req = get_message<m3::KIF::Syscall::CreateSess>(msg);
    capsel_t dst = req->dst_sel;
    capsel_t srv = req->srv_sel;
    word_t ident = req->ident;

    LOG_SYS(vpe, ": syscall::create_sess",
        "(dst=" << dst << ", srv=" << srv << ", ident=#" << m3::fmt(ident, "0x") << ")");

    if(!vpe->objcaps().unused(dst))
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid session selector");

    auto srvcap = static_cast<ServCapability*>(vpe->objcaps().get(srv, Capability::SERV));
    if(srvcap == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Service capability is invalid");

    auto sesscap = SYS_CREATE_CAP(vpe, msg, SessCapability, SessObject,
        &vpe->objcaps(), dst, const_cast<Service*>(&*srvcap->obj), ident);
    vpe->objcaps().inherit(srvcap, sesscap);
    vpe->objcaps().set(dst, sesscap);

    reply_result(vpe, msg, m3::Errors::NONE);
}

void SyscallHandler::create_rgate(VPE *vpe, const m3::DTU::Message *msg) {
    EVENT_TRACER_Syscall_creatergate();

    auto req = get_message<m3::KIF::Syscall::CreateRGate>(msg);
    capsel_t dst = req->dst_sel;
    int order = req->order;
    int msgorder = req->msgorder;

    LOG_SYS(vpe, ": syscall::create_rgate", "(dst=" << dst
        << ", size=" << m3::fmt(1UL << order, "#x")
        << ", msgsize=" << m3::fmt(1UL << msgorder, "#x") << ")");

    if(!vpe->objcaps().unused(dst))
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid rgate selector");

    if(msgorder + order < msgorder || msgorder > order || order - msgorder >= 32)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid arguments");
    if((1UL << (order - msgorder)) > MAX_RB_SIZE)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Too many receive buffer slots");

    auto rgatecap = SYS_CREATE_CAP(vpe, msg, RGateCapability, RGateObject,
        &vpe->objcaps(), dst, order, msgorder);
    vpe->objcaps().set(dst, rgatecap);

    reply_result(vpe, msg, m3::Errors::NONE);
}

void SyscallHandler::create_sgate(VPE *vpe, const m3::DTU::Message *msg) {
    EVENT_TRACER_Syscall_createsgate();

    auto req = get_message<m3::KIF::Syscall::CreateSGate>(msg);
    capsel_t dst = req->dst_sel;
    capsel_t rgate = req->rgate_sel;
    label_t label = req->label;
    word_t credits = req->credits;

    LOG_SYS(vpe, ": syscall::create_sgate", "(dst=" << dst << ", rgate=" << rgate
        << ", label=" << m3::fmt(label, "#0x", sizeof(label_t) * 2)
        << ", crd=#" << m3::fmt(credits, "0x") << ")");

    auto rgatecap = static_cast<RGateCapability*>(vpe->objcaps().get(rgate, Capability::RGATE));
    if(rgatecap == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "RGate capability is invalid");

    if(!vpe->objcaps().unused(dst))
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid cap");

    auto sgcap = SYS_CREATE_CAP(vpe, msg, SGateCapability, SGateObject,
        &vpe->objcaps(), dst, &*rgatecap->obj, label, credits);
    vpe->objcaps().inherit(rgatecap, sgcap);
    vpe->objcaps().set(dst, sgcap);

    reply_result(vpe, msg, m3::Errors::NONE);
}

void SyscallHandler::create_vgroup(VPE *vpe, const m3::DTU::Message *msg) {
    auto req = get_message<m3::KIF::Syscall::CreateVPEGrp>(msg);
    capsel_t dst = req->dst_sel;

    LOG_SYS(vpe, ": syscall::create_vgroup", "(dst=" << dst << ")");

    if(!vpe->objcaps().unused(dst))
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid cap");

    auto vpegcap = SYS_CREATE_CAP(vpe, msg, VPEGroupCapability, VPEGroup, &vpe->objcaps(), dst);
    vpe->objcaps().set(dst, vpegcap);

    reply_result(vpe, msg, m3::Errors::NONE);
}

void SyscallHandler::create_vpe(VPE *vpe, const m3::DTU::Message *msg) {
    EVENT_TRACER_Syscall_createvpe();

    auto req = get_message<m3::KIF::Syscall::CreateVPE>(msg);
    m3::KIF::CapRngDesc dst(req->dst_crd);
    capsel_t sgate = req->sgate_sel;
    m3::PEDesc::value_t pe = req->pe;
    epid_t sep = req->sep;
    epid_t rep = req->rep;
    uint flags = req->flags;
    capsel_t group = req->group_sel;
    capsel_t kmem = req->kmem_sel;
    m3::String name(req->name, m3::Math::min(static_cast<size_t>(req->namelen), sizeof(req->name)));

    LOG_SYS(vpe, ": syscall::create_vpe", "(dst=" << dst << ", sgate=" << sgate << ", name=" << name
        << ", pe=" << static_cast<int>(m3::PEDesc(pe).type())
        << ", sep=" << sep << ", rep=" << rep << ", flags=" << flags
        << ", group=" << group << ", kmem=" << kmem << ")");

    capsel_t capnum = m3::KIF::FIRST_FREE_SEL;
    if(dst.count() != capnum || !vpe->objcaps().range_unused(dst))
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid destination CRD");
    if(name.length() == 0)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid name");

    // if it has a pager, we need an sgate cap
    SGateCapability *sgatecap = nullptr;
    if(sgate != m3::KIF::INV_SEL) {
        sgatecap = static_cast<SGateCapability*>(vpe->objcaps().get(sgate, Capability::SGATE));
        if(sgatecap == nullptr)
            SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid SendGate cap(s)");
        if(sep >= EP_COUNT || sep < m3::DTU::FIRST_FREE_EP)
            SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid SEP");
    }
    else
        sep = VPE::INVALID_EP;

    // check REP
    if(rep != 0 && rep >= EP_COUNT)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid REP");

    VPEGroup *vpegrp = nullptr;
    if(group != m3::KIF::INV_SEL) {
        auto vpegrpcap = static_cast<VPEGroupCapability*>(vpe->objcaps().get(group, Capability::VPEGRP));
        if(vpegrpcap == nullptr)
            SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid VPEGroup cap");
        vpegrp = &*vpegrpcap->obj;
    }

    auto kmemcap = static_cast<KMemCapability*>(vpe->objcaps().get(kmem, Capability::KMEM));
    if(kmemcap == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid KMem cap");

    // the parent gets all caps from the child
    if(!vpe->kmem()->has_quota(capnum * sizeof(SGateCapability)))
        SYS_ERROR(vpe, msg, m3::Errors::NO_KMEM, "Out of kernel memory");
    // the child quota needs to be sufficient
    if(!kmemcap->obj->has_quota(VPE::base_kmem() + VPE::extra_kmem(m3::PEDesc(pe))))
        SYS_ERROR(vpe, msg, m3::Errors::NO_KMEM, "Out of kernel memory");

    // create VPE
    VPE *nvpe = VPEManager::get().create(m3::Util::move(name), m3::PEDesc(pe),
        sep, rep, sgate, &*kmemcap->obj, flags, vpegrp);
    if(nvpe == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::NO_FREE_PE, "No free and suitable PE found");

    // inherit VPE, mem, and EP caps to the parent
    for(capsel_t i = 0; i < capnum; ++i)
        vpe->objcaps().obtain(dst.start() + i, nvpe->objcaps().get(i));

    // delegate pf gate to the new VPE
    if(sgate != m3::KIF::INV_SEL)
        nvpe->objcaps().obtain(sgate, sgatecap);
    nvpe->objcaps().obtain(kmem, kmemcap);

    m3::KIF::Syscall::CreateVPEReply reply;
    reply.error = m3::Errors::NONE;
    reply.pe = Platform::pe(nvpe->pe()).value();
    reply_msg(vpe, msg, &reply, sizeof(reply));
}

void SyscallHandler::create_map(VPE *vpe, const m3::DTU::Message *msg) {
    EVENT_TRACER_Syscall_createmap();

#if defined(__gem5__)
    auto req = get_message<m3::KIF::Syscall::CreateMap>(msg);
    capsel_t dst = req->dst_sel;
    capsel_t mgate = req->mgate_sel;
    capsel_t tvpe = req->vpe_sel;
    capsel_t first = req->first;
    capsel_t pages = req->pages;
    int perms = req->perms;

    LOG_SYS(vpe, ": syscall::create_map", "(dst=" << dst << ", tvpe=" << tvpe << ", mgate=" << mgate
        << ", first=" << first << ", pages=" << pages << ", perms=" << perms << ")");

    auto vpecap = static_cast<VPECapability*>(vpe->objcaps().get(tvpe, Capability::VIRTPE));
    if(vpecap == nullptr || !Platform::pe(vpecap->obj->pe()).has_virtmem())
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "VPE capability is invalid");
    auto mgatecap = static_cast<MGateCapability*>(vpe->objcaps().get(mgate, Capability::MGATE));
    if(mgatecap == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Memory capability is invalid");

    if((mgatecap->obj->addr & PAGE_MASK) || (mgatecap->obj->size & PAGE_MASK))
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Memory capability is not page aligned");
    if(perms & ~mgatecap->obj->perms)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid permissions");

    size_t total = mgatecap->obj->size >> PAGE_BITS;
    if(first >= total || first + pages <= first || first + pages > total)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Region of memory capability is invalid");

    gaddr_t phys = m3::DTU::build_gaddr(mgatecap->obj->pe, mgatecap->obj->addr + PAGE_SIZE * first);
    CapTable &mcaps = vpecap->obj->mapcaps();

    // check for the max. amount of memory we need for PTs to avoid failures during the mapping
    VPE &vpeobj = *vpecap->obj;
    size_t ptmem = vpeobj.address_space()->max_kmem_for(pages * PAGE_SIZE);
    if(!vpeobj.kmem()->has_quota(ptmem))
        SYS_ERROR(vpe, msg, m3::Errors::NO_KMEM, "Out of kernel memory");

    auto mapcap = static_cast<MapCapability*>(mcaps.get(dst, Capability::MAP));
    if(mapcap == nullptr) {
        if(!vpeobj.kmem()->alloc(vpeobj, sizeof(MapObject) + sizeof(MapCapability)))
            SYS_ERROR(vpe, msg, m3::Errors::NO_KMEM, "Out of kernel memory");

        auto mapcap = new MapCapability(&mcaps, dst, pages, new MapObject(phys, perms));
        mcaps.inherit(mgatecap, mapcap);
        mcaps.set(dst, mapcap);
    }
    else {
        if(mapcap->obj->attr & MapCapability::KERNEL)
            SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Map capability refers to a kernel mapping");
        if(mapcap->length() != pages) {
            SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS,
                "Map capability exists with different number of pages ("
                    << mapcap->length() << " vs. " << pages << ")");
        }
        mapcap->remap(phys, perms);
    }
#endif

    reply_result(vpe, msg, m3::Errors::NONE);
}

void SyscallHandler::activate(VPE *vpe, const m3::DTU::Message *msg) {
    EVENT_TRACER_Syscall_activate();

    auto *req = get_message<m3::KIF::Syscall::Activate>(msg);
    capsel_t ep = req->ep_sel;
    capsel_t gate = req->gate_sel;
    goff_t addr = req->addr;

    LOG_SYS(vpe, ": syscall::activate", "(ep=" << ep << ", gate=" << gate
        << ", addr=#" << m3::fmt(addr, "x") << ")");

    auto epcap = static_cast<EPCapability*>(vpe->objcaps().get(ep, Capability::EP));
    if(epcap == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "EP capability is invalid");

    VPE &dstvpe = VPEManager::get().vpe(epcap->obj->vpe);

    GateObject *gateobj = nullptr;
    if(gate != m3::KIF::INV_SEL) {
        auto gatecap = vpe->objcaps().get(gate, Capability::SGATE | Capability::MGATE | Capability::RGATE);
        if(gatecap == nullptr)
            SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid capability");
        gateobj = gatecap->as_gate();
    }

    bool invalid = false;
    if(epcap->obj->gate) {
        if(epcap->obj->gate->type == Capability::RGATE)
            static_cast<RGateObject*>(epcap->obj->gate)->addr = 0;
        // the remote invalidation is only required for send gates
        else if(epcap->obj->gate->type == Capability::SGATE) {
            if(!dstvpe.invalidate_ep(epcap->obj->ep))
                SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Unable to invalidate EP");
            static_cast<SGateObject*>(epcap->obj->gate)->activated = false;
            invalid = true;
        }

        if(gateobj != epcap->obj->gate) {
            epcap->obj->gate->remove_ep(&*epcap->obj);
            epcap->obj->gate = nullptr;
        }
    }

    if(gateobj) {
        EPObject *oldep = gateobj->ep_of_vpe(dstvpe.id());
        if(oldep && oldep->ep != epcap->obj->ep)
            SYS_ERROR(vpe, msg, m3::Errors::EXISTS, "Capability already in use");

        if(gateobj->type == Capability::MGATE) {
            auto mgateobj = static_cast<MGateObject*>(gateobj);
            m3::Errors::Code res = dstvpe.config_mem_ep(epcap->obj->ep, *mgateobj, addr);
            if(res != m3::Errors::NONE)
                SYS_ERROR(vpe, msg, res, "Unable to configure memory EP");
        }
        else if(gateobj->type == Capability::SGATE) {
            auto sgateobj = static_cast<SGateObject*>(gateobj);

            if(!sgateobj->rgate->activated()) {
                LOG_SYS(vpe, ": syscall::activate",
                    ": waiting for rgate " << &sgateobj->rgate);

                vpe->start_wait();
                m3::ThreadManager::get().wait_for(reinterpret_cast<event_t>(&*sgateobj->rgate));
                vpe->stop_wait();

                LOG_SYS(vpe, ": syscall::activate-cont",
                    ": rgate " << &sgateobj->rgate << " activated");

                // ensure that dstvpe is still valid
                if(vpe->objcaps().get(ep, Capability::EP) == nullptr)
                    SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "EP was revoked");
            }

            m3::Errors::Code res = dstvpe.config_snd_ep(epcap->obj->ep, *sgateobj);
            if(res != m3::Errors::NONE)
                SYS_ERROR(vpe, msg, res, "Unable to configure send EP");
        }
        else {
            auto rgateobj = static_cast<RGateObject*>(gateobj);
            if(rgateobj->activated())
                SYS_ERROR(vpe, msg, m3::Errors::EXISTS, "RGate already activated");

            rgateobj->vpe = dstvpe.id();
            rgateobj->addr = addr;
            rgateobj->ep = epcap->obj->ep;

            m3::Errors::Code res = dstvpe.config_rcv_ep(epcap->obj->ep, *rgateobj);
            if(res != m3::Errors::NONE) {
                rgateobj->addr = 0;
                SYS_ERROR(vpe, msg, res, "Unable to configure receive EP");
            }
        }

        if(!oldep)
            gateobj->add_ep(&*epcap->obj);
    }
    else {
        if(!invalid && !dstvpe.invalidate_ep(epcap->obj->ep))
            SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Unable to invalidate EP");
    }

    epcap->obj->gate = gateobj;

    reply_result(vpe, msg, m3::Errors::NONE);
}

void SyscallHandler::vpe_ctrl(VPE *vpe, const m3::DTU::Message *msg) {
    EVENT_TRACER_Syscall_vpectrl();

    auto req = get_message<m3::KIF::Syscall::VPECtrl>(msg);
    capsel_t tvpe = req->vpe_sel;
    m3::KIF::Syscall::VPEOp op = static_cast<m3::KIF::Syscall::VPEOp>(req->op);
    word_t arg = req->arg;

    static const char *opnames[] = {
        "INIT", "START", "YIELD", "STOP"
    };

    LOG_SYS(vpe, ": syscall::vpe_ctrl", "(vpe=" << tvpe
        << ", op=" << (static_cast<size_t>(op) < ARRAY_SIZE(opnames) ? opnames[op] : "??")
        << ", arg=" << m3::fmt(arg, "#x") << ")");

    auto vpecap = static_cast<VPECapability*>(vpe->objcaps().get(tvpe, Capability::VIRTPE));
    if(vpecap == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid VPE cap");

    switch(op) {
        case m3::KIF::Syscall::VCTRL_INIT:
            vpecap->obj->set_mem_base(arg);
            break;

        case m3::KIF::Syscall::VCTRL_START:
            if(vpe == &*vpecap->obj)
                SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "VPE can't start itself");
            vpecap->obj->start_app(static_cast<int>(arg));
            break;

        case m3::KIF::Syscall::VCTRL_YIELD:
            if(vpe != &*vpecap->obj)
                SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Yield for other VPEs is prohibited");

            // reply before the context switch
            reply_result(vpe, msg, m3::Errors::NONE);
            vpecap->obj->yield();
            return;

        case m3::KIF::Syscall::VCTRL_STOP: {
            bool self = vpe == &*vpecap->obj;
            vpecap->obj->stop_app(static_cast<int>(arg), self);
            if(self) {
                // if we don't reply, we need to mark it read manually
                m3::DTU::get().mark_read(vpe->syscall_ep(), reinterpret_cast<uintptr_t>(msg));
                return;
            }
            break;
        }
    }

    reply_result(vpe, msg, m3::Errors::NONE);
}

void SyscallHandler::vpe_wait(VPE *vpe, const m3::DTU::Message *msg) {
    auto req = get_message<m3::KIF::Syscall::VPEWait>(msg);
    size_t count = req->vpe_count;
    event_t event = req->event;
    xfer_t sels_cpy[ARRAY_SIZE(req->sels)];
    const xfer_t *sels = req->sels;

    if(count == 0 || count > ARRAY_SIZE(req->sels))
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "VPE count is invalid");

    m3::KIF::Syscall::VPEWaitReply reply;
    reply.error = m3::Errors::NONE;

    LOG_SYS(vpe, ": syscall::vpe_wait", "(vpes=" << count << ")");

    // copy it from the message if we reply via upcall, because the message may be overwritten
    if(event) {
        memcpy(sels_cpy, sels, sizeof(sels_cpy));
        sels = sels_cpy;
        reply_result(vpe, msg, m3::Errors::NONE);
    }

    while(true) {
        for(size_t i = 0; i < count; ++i) {
            auto vpecap = static_cast<VPECapability*>(vpe->objcaps().get(sels[i], Capability::VIRTPE));
            if(vpecap == nullptr || &*vpecap->obj == vpe)
                continue;

            if(!vpecap->obj->has_app()) {
                reply.vpe_sel = sels[i];
                reply.exitcode = static_cast<xfer_t>(vpecap->obj->exitcode());
                goto done;
            }
        }

        vpe->start_wait();
        VPE::wait_for_exit();
        vpe->stop_wait();
    }

done:
    LOG_SYS(vpe, ": syscall::vpe_wait-cont",
        "(vpe=" << reply.vpe_sel << ", exitcode=" << reply.exitcode << ")");

    if(event)
        vpe->upcall_vpewait(event, reply);
    else
        reply_msg(vpe, msg, &reply, sizeof(reply));
}

void SyscallHandler::derive_mem(VPE *vpe, const m3::DTU::Message *msg) {
    EVENT_TRACER_Syscall_derivemem();

    auto req = get_message<m3::KIF::Syscall::DeriveMem>(msg);
    capsel_t tvpe = req->vpe_sel;
    capsel_t dst = req->dst_sel;
    capsel_t src = req->src_sel;
    goff_t offset = req->offset;
    size_t size = req->size;
    int perms = req->perms;

    LOG_SYS(vpe, ": syscall::derive_mem", "(vpe=" << tvpe << ", src=" << src << ", dst=" << dst
        << ", size=" << size << ", off=" << offset << ", perms=" << perms << ")");

    auto vpecap = static_cast<VPECapability*>(vpe->objcaps().get(tvpe, Capability::VIRTPE));
    if(vpecap == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid VPE cap");

    auto srccap = static_cast<MGateCapability*>(vpe->objcaps().get(src, Capability::MGATE));
    if(srccap == nullptr || !vpecap->obj->objcaps().unused(dst))
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid cap(s)");

    if(offset + size < offset || offset + size > srccap->obj->size || size == 0 ||
            (perms & ~(m3::KIF::Perm::RWX)))
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid args");

    auto dercap = SYS_CREATE_CAP(vpe, msg, MGateCapability, MGateObject,
        &vpecap->obj->objcaps(), dst,
        srccap->obj->pe,
        srccap->obj->vpe,
        srccap->obj->addr + offset,
        size,
        perms & srccap->obj->perms
    );
    vpecap->obj->objcaps().inherit(srccap, dercap);
    vpecap->obj->objcaps().set(dst, dercap);

    reply_result(vpe, msg, m3::Errors::NONE);
}

void SyscallHandler::derive_kmem(VPE *vpe, const m3::DTU::Message *msg) {
    auto req = get_message<m3::KIF::Syscall::DeriveKMem>(msg);
    capsel_t kmem = req->kmem_sel;
    capsel_t dst = req->dst_sel;
    size_t quota = req->quota;

    LOG_SYS(vpe, ": syscall::derive_kmem", "(kmem=" << kmem << ", dst=" << dst
        << ", quota=" << quota << ")");

    auto kmemcap = static_cast<KMemCapability*>(vpe->objcaps().get(kmem, Capability::KMEM));
    if(kmemcap == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid KMem cap");

    if(!kmemcap->obj->alloc(*vpe, quota))
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Insufficient quota");

    auto dercap = SYS_CREATE_CAP(vpe, msg, KMemCapability, KMemObject,
        &vpe->objcaps(), dst,
        quota
    );
    vpe->objcaps().inherit(kmemcap, dercap);
    vpe->objcaps().set(dst, dercap);

    reply_result(vpe, msg, m3::Errors::NONE);
}

void SyscallHandler::kmem_quota(VPE *vpe, const m3::DTU::Message *msg) {
    auto req = get_message<m3::KIF::Syscall::KMemQuota>(msg);
    capsel_t kmem = req->kmem_sel;

    LOG_SYS(vpe, ": syscall::kmem_quota", "(kmem=" << kmem << ")");

    auto kmemcap = static_cast<KMemCapability*>(vpe->objcaps().get(kmem, Capability::KMEM));
    if(kmemcap == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid KMem cap");

    m3::KIF::Syscall::KMemQuotaReply reply;
    reply.error = m3::Errors::NONE;
    reply.amount = kmemcap->obj->left;
    reply_msg(vpe, msg, &reply, sizeof(reply));
}

void SyscallHandler::delegate(VPE *vpe, const m3::DTU::Message *msg) {
    EVENT_TRACER_Syscall_delegate();
    exchange_over_sess(vpe, msg, false);
}

void SyscallHandler::obtain(VPE *vpe, const m3::DTU::Message *msg) {
    EVENT_TRACER_Syscall_obtain();
    exchange_over_sess(vpe, msg, true);
}

void SyscallHandler::exchange(VPE *vpe, const m3::DTU::Message *msg) {
    EVENT_TRACER_Syscall_exchange();

    auto req = get_message<m3::KIF::Syscall::Exchange>(msg);
    capsel_t tvpe = req->vpe_sel;
    m3::KIF::CapRngDesc own(req->own_crd);
    m3::KIF::CapRngDesc other(own.type(), req->other_sel, own.count());
    bool obtain = req->obtain;

    LOG_SYS(vpe, ": syscall::exchange", "(vpe=" << tvpe << ", own=" << own
        << ", other=" << other << ", obtain=" << obtain << ")");

    auto vpecap = static_cast<VPECapability*>(vpe->objcaps().get(tvpe, Capability::VIRTPE));
    if(vpecap == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid VPE cap");

    m3::Errors::Code res = do_exchange(vpe, &*vpecap->obj, own, other, obtain);

    reply_result(vpe, msg, res);
}

void SyscallHandler::revoke(VPE *vpe, const m3::DTU::Message *msg) {
    EVENT_TRACER_Syscall_revoke();

    auto *req = get_message<m3::KIF::Syscall::Revoke>(msg);
    capsel_t tvpe = req->vpe_sel;
    m3::KIF::CapRngDesc crd(req->crd);
    bool own = req->own;

    LOG_SYS(vpe, ": syscall::revoke", "(vpe=" << tvpe << ", crd=" << crd << ", own=" << own << ")");

    auto vpecap = static_cast<VPECapability*>(vpe->objcaps().get(tvpe, Capability::VIRTPE));
    if(vpecap == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid cap");

    if(crd.type() == m3::KIF::CapRngDesc::OBJ && crd.start() < 2)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Cap 0 and 1 are not revokeable");

    if(crd.type() == m3::KIF::CapRngDesc::OBJ)
        vpecap->obj->objcaps().revoke(crd, own);
    else
        vpecap->obj->mapcaps().revoke(crd, own);

    reply_result(vpe, msg, m3::Errors::NONE);
}

m3::Errors::Code SyscallHandler::do_exchange(VPE *v1, VPE *v2, const m3::KIF::CapRngDesc &c1,
                                             const m3::KIF::CapRngDesc &c2, bool obtain) {
    VPE &src = obtain ? *v2 : *v1;
    VPE &dst = obtain ? *v1 : *v2;
    const m3::KIF::CapRngDesc &srcrng = obtain ? c2 : c1;
    const m3::KIF::CapRngDesc &dstrng = obtain ? c1 : c2;

    if(c1.type() != c2.type()) {
        LOG_ERROR(v1, m3::Errors::INV_ARGS, "Descriptor types don't match");
        return m3::Errors::INV_ARGS;
    }
    if((obtain && c2.count() > c1.count()) || (!obtain && c2.count() != c1.count())) {
        LOG_ERROR(v1, m3::Errors::INV_ARGS, "Server gave me invalid CRD");
        return m3::Errors::INV_ARGS;
    }
    if(!dst.objcaps().range_unused(dstrng)) {
        LOG_ERROR(v1, m3::Errors::INV_ARGS, "Invalid destination caps: " << dstrng);
        return m3::Errors::INV_ARGS;
    }

    // TODO exchange map caps doesn't really work yet, because they might have a length > 1

    CapTable &srctab = c1.type() == m3::KIF::CapRngDesc::OBJ ? src.objcaps() : src.mapcaps();
    CapTable &dsttab = c1.type() == m3::KIF::CapRngDesc::OBJ ? dst.objcaps() : dst.mapcaps();
    for(uint i = 0; i < c2.count(); ++i) {
        capsel_t srcsel = srcrng.start() + i;
        capsel_t dstsel = dstrng.start() + i;
        Capability *srccap = srctab.get(srcsel);
        assert(dsttab.get(dstsel) == nullptr);
        dsttab.obtain(dstsel, srccap);
    }
    return m3::Errors::NONE;
}

void SyscallHandler::exchange_over_sess(VPE *vpe, const m3::DTU::Message *msg, bool obtain) {
    auto req = get_message<m3::KIF::Syscall::ExchangeSess>(msg);
    capsel_t vpe_sel = req->vpe_sel;
    capsel_t sess = req->sess_sel;
    m3::KIF::CapRngDesc crd(req->crd);

    LOG_SYS(vpe, (obtain ? ": syscall::obtain" : ": syscall::delegate"),
            "(vpe=" << vpe_sel << ", sess=" << sess << ", crd=" << crd << ")");

    auto vpecap = static_cast<VPECapability*>(vpe->objcaps().get(vpe_sel, Capability::VIRTPE));
    if(vpecap == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid VPE cap");

    auto sesscap = static_cast<SessCapability*>(vpe->objcaps().get(sess, Capability::SESS));
    if(sesscap == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid session cap");

    // we can't be sure that the session and the VPE still exist when we receive the reply
    m3::Reference<Service> rsrv(sesscap->obj->srv);
    m3::Reference<VPE> rvpe(vpe);

    vpe->start_wait();
    while(rsrv->vpe().state() != VPE::RUNNING) {
        rsrv->vpe().migrate_for(vpe);
        if(!rsrv->vpe().resume()) {
            vpe->stop_wait();
            SYS_ERROR(vpe, msg, m3::Errors::VPE_GONE, "VPE does no longer exist");
        }
    }

    m3::KIF::Service::Exchange smsg;
    smsg.opcode = obtain ? m3::KIF::Service::OBTAIN : m3::KIF::Service::DELEGATE;
    smsg.sess = sesscap->obj->ident;
    smsg.data.caps = crd.count();
    memcpy(&smsg.data.args, &req->args, sizeof(req->args));

    const m3::DTU::Message *srvreply = rsrv->send_receive(smsg.sess, &smsg, sizeof(smsg), false);
    vpe->stop_wait();
    // if the VPE exited, we don't even want to reply
    if(!vpe->has_app()) {
        // due to the missing reply, we need to ack the msg explicitly
        m3::DTU::get().mark_read(vpe->syscall_ep(), reinterpret_cast<uintptr_t>(msg));
        LOG_ERROR(vpe, m3::Errors::VPE_GONE, "Client died during cap exchange");
        return;
    }

    if(srvreply == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::RECV_GONE, "Service unreachable");

    auto *reply = reinterpret_cast<const m3::KIF::Service::ExchangeReply*>(srvreply->data);

    m3::Errors::Code res = static_cast<m3::Errors::Code>(reply->error);

    LOG_SYS(vpe, (obtain ? ": syscall::obtain-cont" : ": syscall::delegate-cont"), "(res=" << res << ")");

    if(res != m3::Errors::NONE)
        LOG_ERROR(vpe, res, "Server denied cap-transfer");
    else {
        m3::KIF::CapRngDesc srvcaps(reply->data.caps);
        res = do_exchange(&*vpecap->obj, &rsrv->vpe(), crd, srvcaps, obtain);
    }

    m3::KIF::Syscall::ExchangeSessReply kreply;
    kreply.error = static_cast<xfer_t>(res);
    kreply.args.count = 0;
    if(res == m3::Errors::NONE)
        memcpy(&kreply.args, &reply->data.args, sizeof(reply->data.args));
    reply_msg(vpe, msg, &kreply, sizeof(kreply));
}

m3::Errors::Code SyscallHandler::wait_for(const char *name, VPE &tvpe, VPE *cur, bool need_app) {
    m3::Errors::Code res = m3::Errors::NONE;
    bool same_group = cur->group() && cur->group().get() == tvpe.group().get();
    while(res == m3::Errors::NONE && tvpe.state() != VPE::RUNNING) {
        if(!same_group)
            cur->start_wait();
        tvpe.add_forward();

        tvpe.migrate_for(cur);

        LOG_SYS(cur, name, ": waiting for VPE " << tvpe.id() << " at " << tvpe.pe() << ", state=" << tvpe.state());

        if(!tvpe.resume(need_app))
            res = m3::Errors::VPE_GONE;

        tvpe.rem_forward();
        if(!same_group)
            cur->stop_wait();
    }

    LOG_SYS(cur, name, "-cont: VPE " << tvpe.id() << " ready");
    return res;
}

void SyscallHandler::forward_msg(VPE *vpe, const m3::DTU::Message *msg) {
    EVENT_TRACER_Syscall_forwardmsg();

#if !defined(__gem5__)
    reply_result(vpe, msg, m3::Errors::NOT_SUP);
#else
    auto *req = get_message<m3::KIF::Syscall::ForwardMsg>(msg);
    capsel_t sgate = req->sgate_sel;
    capsel_t rgate = req->rgate_sel;
    size_t len = req->len;
    label_t rlabel = req->rlabel;
    word_t event = req->event;
    char msg_cpy[m3::KIF::MAX_MSG_SIZE];
    const char *msg_ptr = req->msg;

    LOG_SYS(vpe, ": syscall::forward_msg", "(sgate=" << sgate << ", rgate=" << rgate
        << ", len=" << len << ", rlabel=" << m3::fmt(rlabel, "0x")
        << ", event=" << m3::fmt(event, "0x") << ")");

    auto sgatecap = static_cast<SGateCapability*>(vpe->objcaps().get(sgate, Capability::SGATE));
    if(sgatecap == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid msg cap");
    epid_t rep = m3::DTU::DEF_REP;
    if(rgate != m3::KIF::INV_SEL) {
        auto rgatecap = static_cast<RGateCapability*>(vpe->objcaps().get(rgate, Capability::RGATE));
        if(rgatecap == nullptr)
            SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid rgate cap");
        rep = rgatecap->obj->ep;
    }

    EPObject *epobj = sgatecap->obj->ep_of_vpe(vpe->id());
    if(epobj == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Msg cap is not activated");
    epid_t ep = epobj->ep;
    if(!vpe->can_forward_msg(ep))
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Send did not fail");

    // TODO if we do that asynchronously, we need to buffer the message somewhere, because the
    // VPE might want to do other system calls in the meantime. probably, VPEs need to allocate
    // the memory beforehand and the kernel will simply use it afterwards.
    VPE &tvpe = VPEManager::get().vpe(sgatecap->obj->rgate->vpe);
    bool async = tvpe.state() != VPE::RUNNING && event;
    if(async) {
        memcpy(msg_cpy, msg_ptr, len);
        msg_ptr = msg_cpy;
        reply_result(vpe, msg, m3::Errors::UPCALL_REPLY);
    }

    m3::Errors::Code res = wait_for(": syscall::forward_msg", tvpe, vpe, true);

    if(res == m3::Errors::NONE) {
        // re-enable the EP first, because the reply to the sent message below might otherwise
        // pass credits back BEFORE we overwrote the EP
        vpe->forward_msg(ep, tvpe.pe(), tvpe.id());

        uint64_t sender = vpe->pe() | (vpe->id() << 8) | (ep << 24) | (static_cast<uint64_t>(rep) << 32);
        res = DTU::get().send_to(tvpe.desc(), sgatecap->obj->rgate->ep, sgatecap->obj->label,
                                 msg_ptr, len, rlabel, rep, sender);
    }
    if(res != m3::Errors::NONE)
        LOG_ERROR(vpe, res, "forward_msg failed");

    if(async)
        vpe->upcall_forward(event, res);
    else
        reply_result(vpe, msg, res);
#endif
}

void SyscallHandler::forward_mem(VPE *vpe, const m3::DTU::Message *msg) {
    EVENT_TRACER_Syscall_forwardmem();

#if !defined(__gem5__)
    reply_result(vpe, msg, m3::Errors::NOT_SUP);
#else
    auto *req = get_message<m3::KIF::Syscall::ForwardMem>(msg);
    capsel_t mgate = req->mgate_sel;
    size_t len = m3::Math::min(sizeof(req->data), static_cast<size_t>(req->len));
    goff_t offset = req->offset;
    uint flags = req->flags;
    word_t event = req->event;
    char msg_cpy[m3::KIF::MAX_MSG_SIZE];
    const char *msg_ptr = req->data;

    LOG_SYS(vpe, ": syscall::forward_mem", "(mgate=" << mgate << ", len=" << len
        << ", offset=" << offset << ", flags=" << m3::fmt(flags, "0x")
        << ", event=" << m3::fmt(event, "0x") << ")");

    auto mgatecap = static_cast<MGateCapability*>(vpe->objcaps().get(mgate, Capability::MGATE));
    if(mgatecap == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid memory cap");

    EPObject *epobj = mgatecap->obj->ep_of_vpe(vpe->id());
    if(epobj == 0)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Mem cap is not activated");
    epid_t ep = epobj->ep;

    if(mgatecap->obj->addr + offset < offset || offset >= mgatecap->obj->size ||
       offset + len < offset || offset + len > mgatecap->obj->size)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid offset/length");
    if((flags & m3::KIF::Syscall::ForwardMem::WRITE) && !(mgatecap->obj->perms & m3::KIF::Perm::W))
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "No write permission");
    if((~flags & m3::KIF::Syscall::ForwardMem::WRITE) && !(mgatecap->obj->perms & m3::KIF::Perm::R))
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "No read permission");

    VPE &tvpe = VPEManager::get().vpe(mgatecap->obj->vpe);
    bool async = tvpe.state() != VPE::RUNNING && event;
    if(async) {
        memcpy(msg_cpy, msg_ptr, len);
        msg_ptr = msg_cpy;
        reply_result(vpe, msg, m3::Errors::UPCALL_REPLY);
    }

    m3::Errors::Code res = wait_for(": syscall::forward_mem", tvpe, vpe, false);

    m3::KIF::Syscall::ForwardMemReply reply;
    reply.error = static_cast<xfer_t>(res);

    if(res == m3::Errors::NONE) {
        if(flags & m3::KIF::Syscall::ForwardMem::WRITE)
            res = DTU::get().try_write_mem(tvpe.desc(), mgatecap->obj->addr + offset, msg_ptr, len);
        else
            res = DTU::get().try_read_mem(tvpe.desc(), mgatecap->obj->addr + offset, reply.data, len);

        vpe->forward_mem(ep, tvpe.pe());
    }
    if(res != m3::Errors::NONE && res != m3::Errors::PAGEFAULT)
        LOG_ERROR(vpe, res, "forward_mem failed");

    if(~flags & m3::KIF::Syscall::ForwardMem::WRITE) {
        if(async)
            UNREACHED; // TODO
        else
            reply_msg(vpe, msg, &reply, sizeof(reply));
    }
    else {
        if(async)
            vpe->upcall_forward(event, res);
        else
            reply_result(vpe, msg, res);
    }
#endif
}

void SyscallHandler::forward_reply(VPE *vpe, const m3::DTU::Message *msg) {
    EVENT_TRACER_Syscall_forwardreply();

#if !defined(__gem5__)
    reply_result(vpe, msg, m3::Errors::NOT_SUP);
#else
    auto *req = get_message<m3::KIF::Syscall::ForwardReply>(msg);
    capsel_t rgate = req->rgate_sel;
    goff_t msgaddr = req->msgaddr;
    size_t len = m3::Math::min(sizeof(req->msg), static_cast<size_t>(req->len));
    word_t event = req->event;
    char msg_cpy[m3::KIF::MAX_MSG_SIZE];
    const char *msg_ptr = req->msg;

    LOG_SYS(vpe, ": syscall::forward_reply", "(rgate=" << rgate << ", len=" << len
        << ", msgaddr=" << (void*)msgaddr << ", event=" << m3::fmt(event, "0x") << ")");

    auto rgatecap = static_cast<RGateCapability*>(vpe->objcaps().get(rgate, Capability::RGATE));
    if(rgatecap == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid rgate cap");

    EPObject *epobj = rgatecap->obj->ep_of_vpe(vpe->id());
    if(epobj == 0)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "RGate cap is not activated");
    epid_t ep = epobj->ep;

    // ensure that the VPE is running, because we need to access it's address space
    while(vpe->state() != VPE::RUNNING) {
        if(!vpe->resume())
            return;
    }

    m3::DTU::ReplyHeader head;
    m3::Errors::Code res = DTU::get().get_header(vpe->desc(), &*rgatecap->obj, msgaddr, &head);
    if(res != m3::Errors::NONE || !(head.flags & m3::DTU::Header::FL_REPLY_FAILED))
        SYS_ERROR(vpe, msg, res, "Invalid arguments");

    // we have read the header. thus, we can mark the message as read (and have to do that before
    // doing the reply)
    DTU::get().mark_read_remote(vpe->desc(), ep, msgaddr);

    // ensure that the VPE still exists since we don't have a capability, but only a message header
    if(!VPEManager::get().exists(head.senderVpeId))
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "VPE does not exist");

    VPE &tvpe = VPEManager::get().vpe(head.senderVpeId);
    bool async = tvpe.state() != VPE::RUNNING && event;
    if(async) {
        memcpy(msg_cpy, msg_ptr, len);
        msg_ptr = msg_cpy;
        reply_result(vpe, msg, m3::Errors::UPCALL_REPLY);
    }

    // on PEs with an MMU, the VMA needs to do message passing even though the application might not
    // be running yet. otherwise, the app needs to be running
    // TODO this is just a stop-gap solution
    bool need_app = !Platform::pe(tvpe.pe()).has_mmu();
    res = wait_for(": syscall::forward_reply", tvpe, vpe, need_app);
    if(res == m3::Errors::NONE) {
        uint64_t sender = vpe->pe() | (vpe->id() << 8) |
                        (static_cast<uint64_t>(head.senderEp) << 32) |
                        (static_cast<uint64_t>(1) << 40);
        res = DTU::get().reply_to(tvpe.desc(), head.replyEp, head.replylabel, msg_ptr, len, sender);
    }
    if(res != m3::Errors::NONE) {
        LOG_ERROR(vpe, res, "forward_reply to "
            << tvpe.id() << ":" << tvpe.name() << "@" << tvpe.pe() << " failed");
    }

    if(async)
        vpe->upcall_forward(event, res);
    else
        reply_result(vpe, msg, res);
#endif
}

void SyscallHandler::noop(VPE *vpe, const m3::DTU::Message *msg) {
    EVENT_TRACER_Syscall_noop();
    LOG_SYS(vpe, ": syscall::noop", "()");

    reply_result(vpe, msg, m3::Errors::NONE);
}

}
