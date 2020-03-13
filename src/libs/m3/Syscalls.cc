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

#include <base/Errors.h>
#include <base/Init.h>

#include <m3/com/GateStream.h>
#include <m3/TCUIf.h>
#include <m3/Exception.h>
#include <m3/Syscalls.h>

namespace m3 {

INIT_PRIO_SYSCALLS SendGate Syscalls::_sendgate(KIF::INV_SEL, ObjCap::KEEP_CAP,
                                                &RecvGate::syscall(),
                                                env()->std_eps_start + TCU::SYSC_SEP_OFF);

template<class T>
Syscalls::SyscallReply<T> Syscalls::send_receive(const void *msg, size_t size) noexcept {
    const TCU::Message *reply = nullptr;
    Errors::Code res = TCUIf::call(_sendgate, msg, size, *_sendgate.reply_gate(), &reply);
    return SyscallReply<T>(res, reply);
}

Errors::Code Syscalls::send_receive_err(const void *msg, size_t size) noexcept {
    auto reply = send_receive<KIF::DefaultReply>(msg, size);
    return static_cast<Errors::Code>(reply.error());
}

void Syscalls::send_receive_throw(const void *msg, size_t size) {
    Errors::Code res = send_receive_err(msg, size);
    if(res != Errors::NONE) {
        auto *syscall = static_cast<const KIF::DefaultRequest*>(msg);
        throw SyscallException(res, static_cast<KIF::Syscall::Operation>(syscall->opcode));
    }
}

void Syscalls::create_srv(capsel_t dst, capsel_t vpe, capsel_t rgate, const String &name) {
    KIF::Syscall::CreateSrv req;
    req.opcode = KIF::Syscall::CREATE_SRV;
    req.dst_sel = dst;
    req.vpe_sel = vpe;
    req.rgate_sel = rgate;
    req.namelen = Math::min(name.length(), sizeof(req.name));
    memcpy(req.name, name.c_str(), req.namelen);
    size_t msgsize = sizeof(req) - sizeof(req.name) + req.namelen;
    send_receive_throw(&req, msgsize);
}

void Syscalls::create_sess(capsel_t dst, capsel_t srv, word_t ident) {
    KIF::Syscall::CreateSess req;
    req.opcode = KIF::Syscall::CREATE_SESS;
    req.dst_sel = dst;
    req.srv_sel = srv;
    req.ident = ident;
    send_receive_throw(&req, sizeof(req));
}

void Syscalls::create_rgate(capsel_t dst, uint order, uint msgorder) {
    KIF::Syscall::CreateRGate req;
    req.opcode = KIF::Syscall::CREATE_RGATE;
    req.dst_sel = dst;
    req.order = static_cast<xfer_t>(order);
    req.msgorder = static_cast<xfer_t>(msgorder);
    send_receive_throw(&req, sizeof(req));
}

void Syscalls::create_sgate(capsel_t dst, capsel_t rgate, label_t label, uint credits) {
    KIF::Syscall::CreateSGate req;
    req.opcode = KIF::Syscall::CREATE_SGATE;
    req.dst_sel = dst;
    req.rgate_sel = rgate;
    req.label = label;
    req.credits = credits;
    send_receive_throw(&req, sizeof(req));
}

void Syscalls::create_map(capsel_t dst, capsel_t vpe, capsel_t mgate, capsel_t first,
                          capsel_t pages, int perms) {
    KIF::Syscall::CreateMap req;
    req.opcode = KIF::Syscall::CREATE_MAP;
    req.dst_sel = dst;
    req.vpe_sel = vpe;
    req.mgate_sel = mgate;
    req.first = first;
    req.pages = pages;
    req.perms = static_cast<xfer_t>(perms);
    send_receive_throw(&req, sizeof(req));
}

epid_t Syscalls::create_vpe(const KIF::CapRngDesc &dst, capsel_t pg_sg, capsel_t pg_rg,
                            const String &name, capsel_t pe, capsel_t kmem) {
    KIF::Syscall::CreateVPE req;
    req.opcode = KIF::Syscall::CREATE_VPE;
    req.dst_crd = dst.value();
    req.pg_sg_sel = pg_sg;
    req.pg_rg_sel = pg_rg;
    req.pe_sel = pe;
    req.kmem_sel = kmem;
    req.namelen = Math::min(name.length(), sizeof(req.name));
    memcpy(req.name, name.c_str(), req.namelen);

    size_t msgsize = sizeof(req) - sizeof(req.name) + req.namelen;
    auto reply = send_receive<KIF::Syscall::CreateVPEReply>(&req, msgsize);

    Errors::Code res = static_cast<Errors::Code>(reply.error());
    if(res != Errors::NONE)
        throw SyscallException(res, KIF::Syscall::CREATE_VPE);
    return reply->eps_start;
}

void Syscalls::create_sem(capsel_t dst, uint value) {
    KIF::Syscall::CreateSem req;
    req.opcode = KIF::Syscall::CREATE_SEM;
    req.dst_sel = dst;
    req.value = value;
    send_receive_throw(&req, sizeof(req));
}

epid_t Syscalls::alloc_ep(capsel_t dst, capsel_t vpe, epid_t ep, uint replies) {
    KIF::Syscall::AllocEP req;
    req.opcode = KIF::Syscall::ALLOC_EPS;
    req.dst_sel = dst;
    req.vpe_sel = vpe;
    req.epid = ep;
    req.replies = replies;

    auto reply = send_receive<KIF::Syscall::AllocEPReply>(&req, sizeof(req));

    Errors::Code res = static_cast<Errors::Code>(reply.error());
    if(res != Errors::NONE)
        throw SyscallException(res, KIF::Syscall::ALLOC_EPS);
    return reply->ep;
}

void Syscalls::activate(capsel_t ep, capsel_t gate, goff_t addr) {
    KIF::Syscall::Activate req;
    req.opcode = KIF::Syscall::ACTIVATE;
    req.ep_sel = ep;
    req.gate_sel = gate;
    req.addr = addr;
    send_receive_throw(&req, sizeof(req));
}

void Syscalls::vpe_ctrl(capsel_t vpe, KIF::Syscall::VPEOp op, xfer_t arg) {
    KIF::Syscall::VPECtrl req;
    req.opcode = KIF::Syscall::VPE_CTRL;
    req.vpe_sel = vpe;
    req.op = static_cast<xfer_t>(op);
    req.arg = arg;
    send_receive_throw(&req, sizeof(req));
}

int Syscalls::vpe_wait(const capsel_t *vpes, size_t count, event_t event, capsel_t *vpe) {
    KIF::Syscall::VPEWait req;
    req.opcode = KIF::Syscall::VPE_WAIT;
    req.vpe_count = count;
    req.event = event;
    for(size_t i = 0; i < count; ++i)
        req.sels[i] = vpes[i];

    auto reply = send_receive<KIF::Syscall::VPEWaitReply>(&req, sizeof(req));

    int exitcode = -1;
    Errors::Code res = static_cast<Errors::Code>(reply.error());
    if(res == Errors::NONE && event == 0) {
        *vpe = reply->vpe_sel;
        exitcode = reply->exitcode;
    }

    if(res != Errors::NONE)
        throw SyscallException(res, KIF::Syscall::VPE_WAIT);
    return exitcode;
}

void Syscalls::derive_mem(capsel_t vpe, capsel_t dst, capsel_t src, goff_t offset,
                          size_t size, int perms) {
    KIF::Syscall::DeriveMem req;
    req.opcode = KIF::Syscall::DERIVE_MEM;
    req.vpe_sel = vpe;
    req.dst_sel = dst;
    req.src_sel = src;
    req.offset = offset;
    req.size = size;
    req.perms = static_cast<xfer_t>(perms);
    send_receive_throw(&req, sizeof(req));
}

void Syscalls::derive_kmem(capsel_t kmem, capsel_t dst, size_t quota) {
    KIF::Syscall::DeriveKMem req;
    req.opcode = KIF::Syscall::DERIVE_KMEM;
    req.kmem_sel = kmem;
    req.dst_sel = dst;
    req.quota = quota;
    send_receive_throw(&req, sizeof(req));
}

void Syscalls::derive_pe(capsel_t pe, capsel_t dst, uint eps) {
    KIF::Syscall::DerivePE req;
    req.opcode = KIF::Syscall::DERIVE_PE;
    req.pe_sel = pe;
    req.dst_sel = dst;
    req.eps = eps;
    send_receive_throw(&req, sizeof(req));
}

size_t Syscalls::kmem_quota(capsel_t kmem) {
    KIF::Syscall::KMemQuota req;
    req.opcode = KIF::Syscall::KMEM_QUOTA;
    req.kmem_sel = kmem;

    auto reply = send_receive<KIF::Syscall::KMemQuotaReply>(&req, sizeof(req));

    Errors::Code res = static_cast<Errors::Code>(reply.error());
    if(res != Errors::NONE)
        throw SyscallException(res, KIF::Syscall::KMEM_QUOTA);
    return reply->amount;
}

uint Syscalls::pe_quota(capsel_t pe) {
    KIF::Syscall::PEQuota req;
    req.opcode = KIF::Syscall::PE_QUOTA;
    req.pe_sel = pe;

    auto reply = send_receive<KIF::Syscall::PEQuotaReply>(&req, sizeof(req));

    Errors::Code res = static_cast<Errors::Code>(reply.error());
    if(res != Errors::NONE)
        throw SyscallException(res, KIF::Syscall::PE_QUOTA);
    return reply->amount;
}

void Syscalls::sem_ctrl(capsel_t sel, KIF::Syscall::SemOp op) {
    KIF::Syscall::SemCtrl req;
    req.opcode = KIF::Syscall::SEM_CTRL;
    req.sem_sel = sel;
    req.op = op;
    send_receive_throw(&req, sizeof(req));
}

void Syscalls::exchange(capsel_t vpe, const KIF::CapRngDesc &own, capsel_t other, bool obtain) {
    KIF::Syscall::Exchange req;
    req.opcode = KIF::Syscall::EXCHANGE;
    req.vpe_sel = vpe;
    req.own_crd = own.value();
    req.other_sel = other;
    req.obtain = obtain;
    send_receive_throw(&req, sizeof(req));
}

void Syscalls::exchange_sess(capsel_t vpe, capsel_t sess, const KIF::CapRngDesc &crd,
                             KIF::ExchangeArgs *args, bool obtain) {
    KIF::Syscall::ExchangeSess req;
    req.opcode = obtain ? KIF::Syscall::OBTAIN : KIF::Syscall::DELEGATE;
    req.vpe_sel = vpe;
    req.sess_sel = sess;
    req.crd = crd.value();
    if(args)
        memcpy(&req.args, args, sizeof(*args));
    else
        req.args.bytes = 0;

    auto reply = send_receive<KIF::Syscall::ExchangeSessReply>(&req, sizeof(req));

    Errors::Code res = static_cast<Errors::Code>(reply.error());
    if(res != Errors::NONE)
        throw SyscallException(res, static_cast<KIF::Syscall::Operation>(req.opcode));
    if(args)
        memcpy(args, &reply->args, sizeof(*args));
}

void Syscalls::delegate(capsel_t vpe, capsel_t sess, const KIF::CapRngDesc &crd,
                        KIF::ExchangeArgs *args) {
    exchange_sess(vpe, sess, crd, args, false);
}

void Syscalls::obtain(capsel_t vpe, capsel_t sess, const KIF::CapRngDesc &crd,
                      KIF::ExchangeArgs *args) {
    exchange_sess(vpe, sess, crd, args, true);
}

void Syscalls::revoke(capsel_t vpe, const KIF::CapRngDesc &crd, bool own) {
    KIF::Syscall::Revoke req;
    req.opcode = KIF::Syscall::REVOKE;
    req.vpe_sel = vpe;
    req.crd = crd.value();
    req.own = own;
    send_receive_throw(&req, sizeof(req));
}

void Syscalls::noop() {
    KIF::Syscall::Noop req;
    req.opcode = KIF::Syscall::NOOP;
    send_receive_throw(&req, sizeof(req));
}

}
