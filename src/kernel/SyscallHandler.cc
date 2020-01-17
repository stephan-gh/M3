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

#include <base/log/Kernel.h>
#include <base/util/Math.h>
#include <base/Init.h>
#include <base/Panic.h>

#include <thread/ThreadManager.h>

#include <utility>

#include "com/Service.h"
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
        KLOG(SYSC, (vpe)->id() << ":" << (vpe)->name() << "@" << m3::fmt((vpe)->peid(), "X")\
            << (sysname) << expr)

#define LOG_ERROR(vpe, error, msg)                                                          \
    do {                                                                                    \
        KLOG(ERR, "\e[37;41m"                                                               \
            << (vpe)->id() << ":" << (vpe)->name() << "@" << m3::fmt((vpe)->peid(), "X")    \
            << ": " << msg << " (" << m3::Errors::to_string(error) << ")\e[0m");            \
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
    // configure both receive buffers (we need to do that manually in the kernel)
    // TODO we also need to make sure that a VPE's syscall slot isn't in use if we suspend it
    for(size_t i = 0; i < SYSC_REP_COUNT; ++i) {
        uint buford = m3::getnextlog2(32) + VPE::SYSC_MSGSIZE_ORD;
        size_t bufsize = static_cast<size_t>(1) << buford;
        DTU::get().recv_msgs(ep(i),reinterpret_cast<uintptr_t>(new uint8_t[bufsize]),
            buford, VPE::SYSC_MSGSIZE_ORD);
    }

    uint buford = m3::nextlog2<1024>::val;
    size_t bufsize = static_cast<size_t>(1) << buford;
    DTU::get().recv_msgs(srvep(), reinterpret_cast<uintptr_t>(new uint8_t[bufsize]),
        buford, m3::nextlog2<256>::val);

    if(PEMux::total_instances() > 32)
        PANIC("At most 32 PEMux instances are supported");
    buford = m3::nextlog2<32>::val + PEMux::PEXC_MSGSIZE_ORD;
    bufsize = static_cast<size_t>(1) << buford;
    DTU::get().recv_msgs(pexep(), reinterpret_cast<uintptr_t>(new uint8_t[bufsize]),
        buford, PEMux::PEXC_MSGSIZE_ORD);

    add_operation(m3::KIF::Syscall::CREATE_SRV,     &SyscallHandler::create_srv);
    add_operation(m3::KIF::Syscall::CREATE_SESS,    &SyscallHandler::create_sess);
    add_operation(m3::KIF::Syscall::CREATE_RGATE,   &SyscallHandler::create_rgate);
    add_operation(m3::KIF::Syscall::CREATE_SGATE,   &SyscallHandler::create_sgate);
    add_operation(m3::KIF::Syscall::CREATE_VPE,     &SyscallHandler::create_vpe);
    add_operation(m3::KIF::Syscall::CREATE_MAP,     &SyscallHandler::create_map);
    add_operation(m3::KIF::Syscall::CREATE_SEM,     &SyscallHandler::create_sem);
    add_operation(m3::KIF::Syscall::ALLOC_EPS,      &SyscallHandler::alloc_ep);
    add_operation(m3::KIF::Syscall::ACTIVATE,       &SyscallHandler::activate);
    add_operation(m3::KIF::Syscall::VPE_CTRL,       &SyscallHandler::vpe_ctrl);
    add_operation(m3::KIF::Syscall::VPE_WAIT,       &SyscallHandler::vpe_wait);
    add_operation(m3::KIF::Syscall::DERIVE_MEM,     &SyscallHandler::derive_mem);
    add_operation(m3::KIF::Syscall::DERIVE_KMEM,    &SyscallHandler::derive_kmem);
    add_operation(m3::KIF::Syscall::DERIVE_PE,      &SyscallHandler::derive_pe);
    add_operation(m3::KIF::Syscall::KMEM_QUOTA,     &SyscallHandler::kmem_quota);
    add_operation(m3::KIF::Syscall::PE_QUOTA,       &SyscallHandler::pe_quota);
    add_operation(m3::KIF::Syscall::SEM_CTRL,       &SyscallHandler::sem_ctrl);
    add_operation(m3::KIF::Syscall::EXCHANGE,       &SyscallHandler::exchange);
    add_operation(m3::KIF::Syscall::DELEGATE,       &SyscallHandler::delegate);
    add_operation(m3::KIF::Syscall::OBTAIN,         &SyscallHandler::obtain);
    add_operation(m3::KIF::Syscall::REVOKE,         &SyscallHandler::revoke);
    add_operation(m3::KIF::Syscall::NOOP,           &SyscallHandler::noop);
}

void SyscallHandler::reply_msg(VPE *vpe, const m3::DTU::Message *msg, const void *reply, size_t size) {
    epid_t ep = vpe->syscall_ep();
    DTU::get().reply(ep, reply, size, msg);
}

void SyscallHandler::reply_result(VPE *vpe, const m3::DTU::Message *msg, m3::Errors::Code code) {
    m3::KIF::DefaultReply reply;
    reply.error = static_cast<xfer_t>(code);
    reply_msg(vpe, msg, &reply, sizeof(reply));
}

void SyscallHandler::handle_message(VPE *vpe, const m3::DTU::Message *msg) {
    auto req = get_message<m3::KIF::DefaultRequest>(msg);
    m3::KIF::Syscall::Operation op = static_cast<m3::KIF::Syscall::Operation>(req->opcode);

    if(static_cast<size_t>(op) < sizeof(_callbacks) / sizeof(_callbacks[0]))
        _callbacks[op](vpe, msg);
    else
        reply_result(vpe, msg, m3::Errors::INV_ARGS);
}

void SyscallHandler::create_srv(VPE *vpe, const m3::DTU::Message *msg) {
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
    if(rgatecap == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "RGate capability invalid");
    if(!rgatecap->obj->activated())
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "RGate capability not activated");

    auto vpecap = static_cast<VPECapability*>(vpe->objcaps().get(tvpe, Capability::VIRTPE));
    if(vpecap == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "VPE capability invalid");

    if(name.length() == 0)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid server name");

    auto servcap = SYS_CREATE_CAP(vpe, msg, ServCapability, Service,
        &vpe->objcaps(), dst, *vpecap->obj, name, rgatecap->obj);
    vpe->objcaps().set(dst, servcap);

    reply_result(vpe, msg, m3::Errors::NONE);
}

void SyscallHandler::create_sess(VPE *vpe, const m3::DTU::Message *msg) {
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
    auto req = get_message<m3::KIF::Syscall::CreateRGate>(msg);
    capsel_t dst = req->dst_sel;
    uint order = req->order;
    uint msgorder = req->msgorder;

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
    auto req = get_message<m3::KIF::Syscall::CreateSGate>(msg);
    capsel_t dst = req->dst_sel;
    capsel_t rgate = req->rgate_sel;
    label_t label = req->label;
    uint credits = req->credits;

    LOG_SYS(vpe, ": syscall::create_sgate", "(dst=" << dst << ", rgate=" << rgate
        << ", label=" << m3::fmt(label, "#0x", sizeof(label_t) * 2)
        << ", crd=" << credits << ")");

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

void SyscallHandler::create_vpe(VPE *vpe, const m3::DTU::Message *msg) {
    auto req = get_message<m3::KIF::Syscall::CreateVPE>(msg);
    m3::KIF::CapRngDesc dst(req->dst_crd);
    capsel_t pg_sg = req->pg_sg_sel;
    capsel_t pg_rg = req->pg_rg_sel;
    capsel_t pe = req->pe_sel;
    capsel_t kmem = req->kmem_sel;
    m3::String name(req->name, m3::Math::min(static_cast<size_t>(req->namelen), sizeof(req->name)));

    LOG_SYS(vpe, ": syscall::create_vpe", "(dst=" << dst << ", pg_sg=" << pg_sg
        << ", pg_rg=" << pg_rg << ", name=" << name << ", pe=" << pe << ", kmem=" << kmem << ")");

    capsel_t capnum = m3::KIF::FIRST_FREE_SEL;
    if(dst.count() != capnum || !vpe->objcaps().range_unused(dst))
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid destination CRD");
    if(name.length() == 0)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid name");

    // if it has a pager, we need sgate/rgate caps
    SGateCapability *sgatecap = nullptr;
    if(pg_sg != m3::KIF::INV_SEL) {
        sgatecap = static_cast<SGateCapability*>(vpe->objcaps().get(pg_sg, Capability::SGATE));
        if(sgatecap == nullptr)
            SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid SendGate cap(s)");
    }
    RGateCapability *rgatecap = nullptr;
    if(pg_rg != m3::KIF::INV_SEL) {
        rgatecap = static_cast<RGateCapability*>(vpe->objcaps().get(pg_rg, Capability::RGATE));
        if(rgatecap == nullptr || rgatecap->obj->activated())
            SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid RecvGate cap(s)");
    }

    auto pecap = static_cast<PECapability*>(vpe->objcaps().get(pe, Capability::PE));
    // TODO later we will allow multiple VPEs on one PE
    if(pecap == nullptr || pecap->obj->vpes > 0)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid PE cap");

    auto kmemcap = static_cast<KMemCapability*>(vpe->objcaps().get(kmem, Capability::KMEM));
    if(kmemcap == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid KMem cap");

    // the parent gets all caps from the child
    if(!vpe->kmem()->has_quota(capnum * sizeof(SGateCapability)))
        SYS_ERROR(vpe, msg, m3::Errors::NO_KMEM, "Out of kernel memory");
    // the child quota needs to be sufficient
    if(!kmemcap->obj->has_quota(VPE::required_kmem()))
        SYS_ERROR(vpe, msg, m3::Errors::NO_KMEM, "Out of kernel memory");

    // create VPE
    VPE *nvpe = VPEManager::get().create(std::move(name), pecap, kmemcap);
    if(nvpe == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::NO_FREE_PE, "No free and suitable PE found");

    // inherit VPE, mem, and EP caps to the parent
    for(capsel_t i = m3::KIF::SEL_VPE; i < capnum; ++i)
        vpe->objcaps().obtain(dst.start() + i, nvpe->objcaps().get(i));

    // activate pager EPs
    auto pemux = PEManager::get().pemux(nvpe->peid());
    if(pg_sg != m3::KIF::INV_SEL) {
        // workaround: remember the endpoint so that we invalidate it on gate destruction
        auto sep = new EPObject(pemux->pe(), nvpe, m3::DTU::PG_SEP, 0);
        nvpe->set_pg_sep(sep);
        pemux->config_snd_ep(m3::DTU::PG_SEP, nvpe->id(), *sgatecap->obj);
        sgatecap->obj->add_ep(sep);
        sep->gate = &*sgatecap->obj;
    }
    if(pg_rg != m3::KIF::INV_SEL) {
        auto rep = new EPObject(pemux->pe(), nvpe, m3::DTU::PG_REP, 1);
        nvpe->set_pg_rep(rep);
        rgatecap->obj->pe = nvpe->peid();
        rgatecap->obj->addr = VMA_RBUF;
        pemux->config_rcv_ep(m3::DTU::PG_REP, nvpe->id(), m3::DTU::NO_REPLIES, *rgatecap->obj);
        rgatecap->obj->add_ep(rep);
        rep->gate = &*rgatecap->obj;
    }

    m3::KIF::Syscall::CreateVPEReply reply;
    reply.error = m3::Errors::NONE;
    reply.pe = Platform::pe(nvpe->peid()).value();
    reply_msg(vpe, msg, &reply, sizeof(reply));
}

void SyscallHandler::create_map(VPE *vpe, const m3::DTU::Message *msg) {

#if defined(__gem5__)
    auto req = get_message<m3::KIF::Syscall::CreateMap>(msg);
    capsel_t dst = req->dst_sel;
    capsel_t mgate = req->mgate_sel;
    capsel_t tvpe = req->vpe_sel;
    capsel_t first = req->first;
    capsel_t pages = req->pages;
    uint perms = req->perms;

    LOG_SYS(vpe, ": syscall::create_map", "(dst=" << dst << ", tvpe=" << tvpe << ", mgate=" << mgate
        << ", first=" << first << ", pages=" << pages << ", perms=" << perms << ")");

    auto vpecap = static_cast<VPECapability*>(vpe->objcaps().get(tvpe, Capability::VIRTPE));
    if(vpecap == nullptr || !Platform::pe(vpecap->obj->peid()).has_virtmem())
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "VPE capability is invalid");
    auto mgatecap = static_cast<MGateCapability*>(vpe->objcaps().get(mgate, Capability::MGATE));
    if(mgatecap == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Memory capability is invalid");

    if((mgatecap->obj->addr & PAGE_MASK) || (mgatecap->obj->size & PAGE_MASK))
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Memory capability is not page aligned");
    if(perms & ~mgatecap->obj->perms)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid permissions");
    // user-mapped memory is always user-accessible
    perms |= m3::DTU::PTE_I;

    size_t total = mgatecap->obj->size >> PAGE_BITS;
    if(first >= total || first + pages <= first || first + pages > total)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Region of memory capability is invalid");

    gaddr_t phys = m3::DTU::build_gaddr(mgatecap->obj->pe, mgatecap->obj->addr + PAGE_SIZE * first);
    CapTable &mcaps = vpecap->obj->mapcaps();

    VPE &vpeobj = *vpecap->obj;
    // TODO check for the max. amount of memory we need for PTs to avoid failures during the mapping
    // size_t ptmem = vpeobj.address_space()->max_kmem_for(pages * PAGE_SIZE);
    // if(!vpeobj.kmem()->has_quota(ptmem))
    //     SYS_ERROR(vpe, msg, m3::Errors::NO_KMEM, "Out of kernel memory");
	if(vpeobj.is_stopped())
        SYS_ERROR(vpe, msg, m3::Errors::VPE_GONE, "VPE is currently being destroyed");
    
    auto mapcap = static_cast<MapCapability*>(mcaps.get(dst, Capability::MAP));
    if(mapcap == nullptr) {
        if(!mcaps.range_unused(m3::KIF::CapRngDesc(m3::KIF::CapRngDesc::MAP, dst, pages)))
            SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Capability range already in use");
        if(!vpeobj.kmem()->alloc(vpeobj, sizeof(MapObject) + sizeof(MapCapability)))
            SYS_ERROR(vpe, msg, m3::Errors::NO_KMEM, "Out of kernel memory");

        auto mapcap = new MapCapability(&mcaps, dst, pages, new MapObject(phys, perms));
        auto res = mapcap->remap(phys, perms);
        if(res != m3::Errors::NONE) {
            delete mapcap;
            SYS_ERROR(vpe, msg, res, "Map failed at PEMux");
        }

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

        auto res = mapcap->remap(phys, perms);
        if(res != m3::Errors::NONE)
            SYS_ERROR(vpe, msg, res, "Map failed at PEMux");
    }
#endif

    reply_result(vpe, msg, m3::Errors::NONE);
}

void SyscallHandler::alloc_ep(VPE *vpe, const m3::DTU::Message *msg) {
    auto req = get_message<m3::KIF::Syscall::AllocEP>(msg);
    capsel_t dst = req->dst_sel;
    capsel_t tvpe = req->vpe_sel;
    epid_t epid = req->epid;
    uint replies = req->replies;

    LOG_SYS(vpe, ": syscall::alloc_ep", "(dst=" << dst << ", vpe="
        << tvpe << ", epid=" << epid << ", replies=" << replies << ")");

    if(!vpe->objcaps().unused(dst))
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid cap");

    uint epcount = 1 + replies;
    auto vpecap = static_cast<VPECapability*>(vpe->objcaps().get(tvpe, Capability::VIRTPE));
    if(vpecap == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid VPE cap");
    if(!vpecap->obj->pe()->has_quota(epcount)) {
        SYS_ERROR(vpe, msg, m3::Errors::NO_SPACE, "PE capability has insufficient EPs (have "
            << vpecap->obj->pe()->eps << ", need " << epcount << ")");
    }

    auto pemux = PEManager::get().pemux(vpecap->obj->peid());

    if(epid == EP_COUNT) {
        epid = pemux->find_eps(epcount);
        if(epid == EP_COUNT)
            SYS_ERROR(vpe, msg, m3::Errors::NO_SPACE, "No " << epcount << " contiguous EPs found");
    }
    else {
        if(epid > EP_COUNT || epid + epcount < epid || epid + epcount > EP_COUNT)
            SYS_ERROR(vpe, msg, m3::Errors::NO_SPACE, "Invalid endpoint id");
        if(!pemux->eps_free(epid, epcount)) {
            SYS_ERROR(vpe, msg, m3::Errors::NO_SPACE,
                "Endpoints " << epid << ".." << (epid + epcount - 1) << " not free");
        }
    }

    auto epcap = SYS_CREATE_CAP(vpe, msg, EPCapability, EPObject,
        &vpe->objcaps(), dst, pemux->pe(), &*vpecap->obj, epid, replies);
    vpe->objcaps().set(dst, epcap);
    vpecap->obj->pe()->alloc(epcount);
    pemux->alloc_eps(epid, epcount);

    m3::KIF::Syscall::AllocEPReply reply;
    reply.error = m3::Errors::NONE;
    reply.ep = epid;
    reply_msg(vpe, msg, &reply, sizeof(reply));
}

void SyscallHandler::create_sem(VPE *vpe, const m3::DTU::Message *msg) {
    auto req = get_message<m3::KIF::Syscall::CreateSem>(msg);
    capsel_t dst = req->dst_sel;
    uint value = req->value;

    LOG_SYS(vpe, ": syscall::create_sem", "(dst=" << dst << ", value=" << value << ")");

    if(!vpe->objcaps().unused(dst))
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid cap");

    auto semcap = SYS_CREATE_CAP(vpe, msg, SemCapability, SemObject,
        &vpe->objcaps(), dst, value);
    vpe->objcaps().set(dst, semcap);

    reply_result(vpe, msg, m3::Errors::NONE);
}

void SyscallHandler::activate(VPE *vpe, const m3::DTU::Message *msg) {
    auto *req = get_message<m3::KIF::Syscall::Activate>(msg);
    capsel_t ep = req->ep_sel;
    capsel_t gate = req->gate_sel;
    goff_t addr = req->addr;

    LOG_SYS(vpe, ": syscall::activate", "(ep=" << ep << ", gate=" << gate
        << ", addr=#" << m3::fmt(addr, "x") << ")");

    auto epcap = static_cast<EPCapability*>(vpe->objcaps().get(ep, Capability::EP));
    if(epcap == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid EP cap");
    if(epcap->obj->vpe == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::VPE_GONE, "VPE is currently being destroyed");

    peid_t dst_pe = epcap->obj->pe->id;
    PEMux *dst_pemux = PEManager::get().pemux(dst_pe);

    GateObject *gateobj = nullptr;
    if(gate != m3::KIF::INV_SEL) {
        auto gatecap = vpe->objcaps().get(gate, Capability::SGATE | Capability::MGATE | Capability::RGATE);
        if(gatecap == nullptr)
            SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid gate cap");
        if(epcap->obj->replies != 0 && gatecap->type() != Capability::RGATE)
            SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Only rgates use EP caps with reply slots");
        gateobj = gatecap->as_gate();
    }

    bool invalid = false;
    if(epcap->obj->gate) {
        if(epcap->obj->gate->type == Capability::RGATE)
            static_cast<RGateObject*>(epcap->obj->gate)->addr = 0;
        // the remote invalidation is only required for send gates
        else if(epcap->obj->gate->type == Capability::SGATE) {
            if(!dst_pemux->invalidate_ep(epcap->obj->ep))
                SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "EP invalidation failed");
            static_cast<SGateObject*>(epcap->obj->gate)->activated = false;
            invalid = true;
        }

        if(gateobj != epcap->obj->gate) {
            epcap->obj->gate->remove_ep(&*epcap->obj);
            epcap->obj->gate = nullptr;
        }
    }

    if(gateobj) {
        EPObject *oldep = gateobj->ep_of_pe(dst_pe);
        if(oldep && oldep->ep != epcap->obj->ep) {
            SYS_ERROR(vpe, msg, m3::Errors::EXISTS,
                "Gate is already activated on PE" << oldep->pe->id << ":EP " << oldep->ep);
        }

        if(gateobj->type == Capability::MGATE) {
            auto mgateobj = static_cast<MGateObject*>(gateobj);
            m3::Errors::Code res = dst_pemux->config_mem_ep(
                epcap->obj->ep, epcap->obj->vpe->id(), *mgateobj, addr);
            if(res != m3::Errors::NONE)
                SYS_ERROR(vpe, msg, res, "Memory EP configuration failed");
        }
        else if(gateobj->type == Capability::SGATE) {
            auto sgateobj = static_cast<SGateObject*>(gateobj);

            if(!sgateobj->rgate->activated()) {
                LOG_SYS(vpe, ": syscall::activate",
                    ": waiting for rgate " << &sgateobj->rgate);

                m3::ThreadManager::get().wait_for(reinterpret_cast<event_t>(&*sgateobj->rgate));

                LOG_SYS(vpe, ": syscall::activate-cont",
                    ": rgate " << &sgateobj->rgate << " activated");

                // ensure that dstvpe is still valid
                if(vpe->objcaps().get(ep, Capability::EP) == nullptr) {
                    SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS,
                        "EP capability was revoked during activate");
                }
            }

            m3::Errors::Code res = dst_pemux->config_snd_ep(
                epcap->obj->ep, epcap->obj->vpe->id(), *sgateobj);
            if(res != m3::Errors::NONE)
                SYS_ERROR(vpe, msg, res, "Send EP configuration failed");
        }
        else {
            auto rgateobj = static_cast<RGateObject*>(gateobj);
            if(rgateobj->activated())
                SYS_ERROR(vpe, msg, m3::Errors::EXISTS, "Receive gate already activated");

            epid_t replies = m3::DTU::NO_REPLIES;
            if(epcap->obj->replies > 0) {
                uint slots = 1U << (rgateobj->order - rgateobj->msgorder);
                if(epcap->obj->replies != slots) {
                    SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS,
                        "EP cap has " << epcap->obj->replies << " reply slots, need " << slots);
                }
                replies = epcap->obj->ep + 1;
            }

            rgateobj->pe = dst_pe;
            rgateobj->addr = addr;
            rgateobj->ep = epcap->obj->ep;

            m3::Errors::Code res = dst_pemux->config_rcv_ep(
                epcap->obj->ep, epcap->obj->vpe->id(), replies, *rgateobj);
            if(res != m3::Errors::NONE) {
                rgateobj->addr = 0;
                SYS_ERROR(vpe, msg, res, "Receive EP configuration failed");
            }
        }

        if(!oldep)
            gateobj->add_ep(&*epcap->obj);
    }
    else {
        if(!invalid && !dst_pemux->invalidate_ep(epcap->obj->ep))
            SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "EP invalidation failed");
    }

    epcap->obj->gate = gateobj;
    reply_result(vpe, msg, m3::Errors::NONE);
}

void SyscallHandler::vpe_ctrl(VPE *vpe, const m3::DTU::Message *msg) {
    auto req = get_message<m3::KIF::Syscall::VPECtrl>(msg);
    capsel_t tvpe = req->vpe_sel;
    m3::KIF::Syscall::VPEOp op = static_cast<m3::KIF::Syscall::VPEOp>(req->op);
    word_t arg = req->arg;

    static const char *opnames[] = {
        "INIT", "START", "STOP"
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

        case m3::KIF::Syscall::VCTRL_STOP: {
            bool self = vpe == &*vpecap->obj;
            vpecap->obj->stop_app(static_cast<int>(arg), self);
            if(self) {
                // if we don't reply, we need to mark it read manually
                m3::DTU::get().ack_msg(vpe->syscall_ep(), msg);
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

    if(count == 0 || count > ARRAY_SIZE(req->sels))
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "VPE count is invalid");

    m3::KIF::Syscall::VPEWaitReply reply;
    reply.error = m3::Errors::NONE;
    reply.vpe_sel = m3::KIF::INV_SEL;

    LOG_SYS(vpe, ": syscall::vpe_wait", "(vpes=" << count << ", event=" << event << ")");

    if(event) {
        // first copy the selectors from the message to the stack
        xfer_t sels_cpy[ARRAY_SIZE(m3::KIF::Syscall::VPEWait::sels)];
        memcpy(sels_cpy, req->sels, count * sizeof(xfer_t));
        // now early-reply to the application; we'll notify it later via upcall
        reply_result(vpe, msg, m3::Errors::NONE);

        vpe->wait_exit_async(sels_cpy, count, reply);
    }
    else {
        while(!vpe->check_exits(req->sels, count, reply))
            ;
    }

    if(reply.vpe_sel != m3::KIF::INV_SEL) {
        LOG_SYS(vpe, ": syscall::vpe_wait-cont",
            "(vpe=" << reply.vpe_sel << ", exitcode=" << reply.exitcode << ")");

        if(event)
            vpe->upcall_vpewait(event, reply);
        else
            reply_msg(vpe, msg, &reply, sizeof(reply));
    }
}

void SyscallHandler::derive_mem(VPE *vpe, const m3::DTU::Message *msg) {
    auto req = get_message<m3::KIF::Syscall::DeriveMem>(msg);
    capsel_t tvpe = req->vpe_sel;
    capsel_t dst = req->dst_sel;
    capsel_t src = req->src_sel;
    goff_t offset = req->offset;
    size_t size = req->size;
    uint perms = req->perms;

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

    if(!vpe->objcaps().unused(dst))
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid cap");

    auto kmemcap = static_cast<KMemCapability*>(vpe->objcaps().get(kmem, Capability::KMEM));
    if(kmemcap == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid KMem cap");

    if(!kmemcap->obj->has_quota(quota))
        SYS_ERROR(vpe, msg, m3::Errors::NO_SPACE, "Insufficient quota");

    auto dercap = SYS_CREATE_CAP(vpe, msg, KMemCapability, KMemObject,
        &vpe->objcaps(), dst,
        quota
    );
    vpe->objcaps().inherit(kmemcap, dercap);
    vpe->objcaps().set(dst, dercap);
    kmemcap->obj->alloc(*vpe, quota);

    reply_result(vpe, msg, m3::Errors::NONE);
}

void SyscallHandler::derive_pe(VPE *vpe, const m3::DTU::Message *msg) {
    auto req = get_message<m3::KIF::Syscall::DerivePE>(msg);
    capsel_t pe = req->pe_sel;
    capsel_t dst = req->dst_sel;
    uint eps = req->eps;

    LOG_SYS(vpe, ": syscall::derive_pe", "(pe=" << pe << ", dst=" << dst
        << ", eps=" << eps << ")");

    if(!vpe->objcaps().unused(dst))
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid cap");

    auto pecap = static_cast<PECapability*>(vpe->objcaps().get(pe, Capability::PE));
    if(pecap == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid PE cap");

    if(!pecap->obj->has_quota(eps))
        SYS_ERROR(vpe, msg, m3::Errors::NO_SPACE, "Insufficient EPs");

    auto dercap = SYS_CREATE_CAP(vpe, msg, PECapability, PEObject,
        &vpe->objcaps(), dst,
        pecap->obj->id, eps
    );
    vpe->objcaps().inherit(pecap, dercap);
    vpe->objcaps().set(dst, dercap);
    pecap->obj->alloc(eps);

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

void SyscallHandler::pe_quota(VPE *vpe, const m3::DTU::Message *msg) {
    auto req = get_message<m3::KIF::Syscall::PEQuota>(msg);
    capsel_t pe = req->pe_sel;

    LOG_SYS(vpe, ": syscall::pe_quota", "(pe=" << pe << ")");

    auto pecap = static_cast<PECapability*>(vpe->objcaps().get(pe, Capability::PE));
    if(pecap == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid PE cap");

    m3::KIF::Syscall::PEQuotaReply reply;
    reply.error = m3::Errors::NONE;
    reply.amount = pecap->obj->eps;
    reply_msg(vpe, msg, &reply, sizeof(reply));
}

void SyscallHandler::sem_ctrl(VPE *vpe, const m3::DTU::Message *msg) {
    auto req = get_message<m3::KIF::Syscall::SemCtrl>(msg);
    capsel_t sem = req->sem_sel;
    auto op = static_cast<m3::KIF::Syscall::SemOp>(req->op);

    static const char *ops[] = {"UP", "DOWN", "?"};

    LOG_SYS(vpe, ": syscall::sem_ctrl", "(sem=" << sem
        << ", op=" << ops[op < ARRAY_SIZE(ops) ? op : 2] << ")");

    auto semcap = static_cast<SemCapability*>(vpe->objcaps().get(sem, Capability::SEM));
    if(semcap == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid sem cap");

    m3::Errors::Code res = m3::Errors::NONE;
    switch(op) {
        case m3::KIF::Syscall::SCTRL_UP:
            semcap->obj->up();
            break;

        case m3::KIF::Syscall::SCTRL_DOWN: {
            res = semcap->obj->down();
            LOG_SYS(vpe, ": syscall::sem_ctrl-cont", "(res=" << res << ")");
            break;
        }
    }

    reply_result(vpe, msg, res);
}

void SyscallHandler::delegate(VPE *vpe, const m3::DTU::Message *msg) {
    exchange_over_sess(vpe, msg, false);
}

void SyscallHandler::obtain(VPE *vpe, const m3::DTU::Message *msg) {
    exchange_over_sess(vpe, msg, true);
}

void SyscallHandler::exchange(VPE *vpe, const m3::DTU::Message *msg) {
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
    auto *req = get_message<m3::KIF::Syscall::Revoke>(msg);
    capsel_t tvpe = req->vpe_sel;
    m3::KIF::CapRngDesc crd(req->crd);
    bool own = req->own;

    LOG_SYS(vpe, ": syscall::revoke", "(vpe=" << tvpe << ", crd=" << crd << ", own=" << own << ")");

    auto vpecap = static_cast<VPECapability*>(vpe->objcaps().get(tvpe, Capability::VIRTPE));
    if(vpecap == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Invalid cap");

    if(crd.type() == m3::KIF::CapRngDesc::OBJ && crd.start() <= m3::KIF::SEL_MEM)
        SYS_ERROR(vpe, msg, m3::Errors::INV_ARGS, "Caps 0, 1, 2, and 3 are not revocable");

    m3::Errors::Code res;
    if(crd.type() == m3::KIF::CapRngDesc::OBJ)
        res = vpecap->obj->objcaps().revoke(crd, own);
    else
        res = vpecap->obj->mapcaps().revoke(crd, own);
    if(res != m3::Errors::NONE)
        SYS_ERROR(vpe, msg, res, "Revoke failed");

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

    m3::KIF::Service::Exchange smsg;
    smsg.opcode = obtain ? m3::KIF::Service::OBTAIN : m3::KIF::Service::DELEGATE;
    smsg.sess = sesscap->obj->ident;
    smsg.data.caps = crd.count();
    memcpy(&smsg.data.args, &req->args, sizeof(req->args));

    const m3::DTU::Message *srvreply = rsrv->send_receive(smsg.sess, &smsg, sizeof(smsg), false);
    // if the VPE exited, we don't even want to reply
    if(!vpe->has_app()) {
        // due to the missing reply, we need to ack the msg explicitly
        m3::DTU::get().ack_msg(vpe->syscall_ep(), msg);
        LOG_ERROR(vpe, m3::Errors::VPE_GONE, "Client died during cap exchange");
        return;
    }

    if(srvreply == nullptr)
        SYS_ERROR(vpe, msg, m3::Errors::RECV_GONE, "Service unreachable");

    auto *reply = reinterpret_cast<const m3::KIF::Service::ExchangeReply*>(srvreply->data);

    m3::Errors::Code res = static_cast<m3::Errors::Code>(reply->error);

    const char *prefix = obtain ? ": syscall::obtain-cont" : ": syscall::delegate-cont";
    if(res != m3::Errors::NONE)
        LOG_ERROR(vpe, res, prefix << ": server denied cap-transfer");
    else {
        m3::KIF::CapRngDesc srvcaps(reply->data.caps);
        LOG_SYS(vpe, prefix, "(res=" << res << ", srvcaps=" << srvcaps << ")");
        res = do_exchange(&*vpecap->obj, &rsrv->vpe(), crd, srvcaps, obtain);
    }

    m3::KIF::Syscall::ExchangeSessReply kreply;
    kreply.error = static_cast<xfer_t>(res);
    kreply.args.count = 0;
    if(res == m3::Errors::NONE)
        memcpy(&kreply.args, &reply->data.args, sizeof(reply->data.args));
    reply_msg(vpe, msg, &kreply, sizeof(kreply));
}

void SyscallHandler::noop(VPE *vpe, const m3::DTU::Message *msg) {
    LOG_SYS(vpe, ": syscall::noop", "()");

    reply_result(vpe, msg, m3::Errors::NONE);
}

}
