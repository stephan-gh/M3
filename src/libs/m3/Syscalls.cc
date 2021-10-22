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
#include <m3/Exception.h>
#include <m3/Syscalls.h>

namespace m3 {

INIT_PRIO_SYSCALLS SendGate Syscalls::_sendgate(KIF::INV_SEL, ObjCap::KEEP_CAP,
                                                &RecvGate::syscall(),
                                                env()->first_std_ep + TCU::SYSC_SEP_OFF);

void Syscalls::reinit() {
    _sendgate.reset_ep(env()->first_std_ep + TCU::SYSC_SEP_OFF);
}

template<class T>
Syscalls::SyscallReply<T> Syscalls::send_receive(const MsgBuf &msg) noexcept {
    const TCU::Message *reply = _sendgate.call(msg);
    return SyscallReply<T>(reply);
}

Errors::Code Syscalls::send_receive_err(const MsgBuf &msg) noexcept {
    try {
        auto reply = send_receive<KIF::DefaultReply>(msg);
        return static_cast<Errors::Code>(reply.error());
    }
    catch(const TCUException &e) {
        return e.code();
    }
}

void Syscalls::send_receive_throw(const MsgBuf &msg) {
    Errors::Code res = send_receive_err(msg);
    if(res != Errors::NONE) {
        auto syscall = msg.get<KIF::DefaultRequest>();
        throw SyscallException(res, static_cast<KIF::Syscall::Operation>(syscall.opcode));
    }
}

void Syscalls::create_srv(capsel_t dst, capsel_t rgate, const String &name, label_t creator) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::CreateSrv>();
    req.opcode = KIF::Syscall::CREATE_SRV;
    req.dst_sel = dst;
    req.rgate_sel = rgate;
    req.creator = creator;
    req.namelen = Math::min(name.length(), sizeof(req.name));
    memcpy(req.name, name.c_str(), req.namelen);
    send_receive_throw(req_buf);
}

void Syscalls::create_sess(capsel_t dst, capsel_t srv, size_t crt, word_t ident, bool auto_close) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::CreateSess>();
    req.opcode = KIF::Syscall::CREATE_SESS;
    req.dst_sel = dst;
    req.srv_sel = srv;
    req.creator = crt;
    req.ident = ident;
    req.auto_close = auto_close;
    send_receive_throw(req_buf);
}

void Syscalls::create_mgate(capsel_t dst, capsel_t vpe, goff_t addr, size_t size, int perms) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::CreateMGate>();
    req.opcode = KIF::Syscall::CREATE_MGATE;
    req.dst_sel = dst;
    req.vpe_sel = vpe;
    req.addr = addr;
    req.size = size;
    req.perms = static_cast<xfer_t>(perms);
    send_receive_throw(req_buf);
}

void Syscalls::create_rgate(capsel_t dst, uint order, uint msgorder) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::CreateRGate>();
    req.opcode = KIF::Syscall::CREATE_RGATE;
    req.dst_sel = dst;
    req.order = static_cast<xfer_t>(order);
    req.msgorder = static_cast<xfer_t>(msgorder);
    send_receive_throw(req_buf);
}

void Syscalls::create_sgate(capsel_t dst, capsel_t rgate, label_t label, uint credits) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::CreateSGate>();
    req.opcode = KIF::Syscall::CREATE_SGATE;
    req.dst_sel = dst;
    req.rgate_sel = rgate;
    req.label = label;
    req.credits = credits;
    send_receive_throw(req_buf);
}

void Syscalls::create_map(capsel_t dst, capsel_t vpe, capsel_t mgate, capsel_t first,
                          capsel_t pages, int perms) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::CreateMap>();
    req.opcode = KIF::Syscall::CREATE_MAP;
    req.dst_sel = dst;
    req.vpe_sel = vpe;
    req.mgate_sel = mgate;
    req.first = first;
    req.pages = pages;
    req.perms = static_cast<xfer_t>(perms);
    send_receive_throw(req_buf);
}

epid_t Syscalls::create_vpe(capsel_t dst, capsel_t pg_sg, capsel_t pg_rg,
                            const String &name, capsel_t pe, capsel_t kmem,
                            vpeid_t *id) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::CreateVPE>();
    req.opcode = KIF::Syscall::CREATE_VPE;
    req.dst_sel = dst;
    req.pg_sg_sel = pg_sg;
    req.pg_rg_sel = pg_rg;
    req.pe_sel = pe;
    req.kmem_sel = kmem;
    req.namelen = Math::min(name.length(), sizeof(req.name));
    memcpy(req.name, name.c_str(), req.namelen);

    auto reply = send_receive<KIF::Syscall::CreateVPEReply>(req_buf);

    Errors::Code res = static_cast<Errors::Code>(reply.error());
    if(res != Errors::NONE)
        throw SyscallException(res, KIF::Syscall::CREATE_VPE);
    *id = reply->id;
    return reply->eps_start;
}

void Syscalls::create_sem(capsel_t dst, uint value) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::CreateSem>();
    req.opcode = KIF::Syscall::CREATE_SEM;
    req.dst_sel = dst;
    req.value = value;
    send_receive_throw(req_buf);
}

epid_t Syscalls::alloc_ep(capsel_t dst, capsel_t vpe, epid_t ep, uint replies) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::AllocEP>();
    req.opcode = KIF::Syscall::ALLOC_EPS;
    req.dst_sel = dst;
    req.vpe_sel = vpe;
    req.epid = ep;
    req.replies = replies;

    auto reply = send_receive<KIF::Syscall::AllocEPReply>(req_buf);

    Errors::Code res = static_cast<Errors::Code>(reply.error());
    if(res != Errors::NONE)
        throw SyscallException(res, KIF::Syscall::ALLOC_EPS);
    return reply->ep;
}

void Syscalls::activate(capsel_t ep, capsel_t gate, capsel_t rbuf_mem, goff_t rbuf_off) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::Activate>();
    req.opcode = KIF::Syscall::ACTIVATE;
    req.ep_sel = ep;
    req.gate_sel = gate;
    req.rbuf_mem = rbuf_mem;
    req.rbuf_off = rbuf_off;
    send_receive_throw(req_buf);
}

void Syscalls::set_pmp(capsel_t pe, capsel_t mgate, epid_t epid) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::SetPMP>();
    req.opcode = KIF::Syscall::SET_PMP;
    req.pe_sel = pe;
    req.mgate_sel = mgate;
    req.epid = epid;
    send_receive_throw(req_buf);
}

void Syscalls::vpe_ctrl(capsel_t vpe, KIF::Syscall::VPEOp op, xfer_t arg) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::VPECtrl>();
    req.opcode = KIF::Syscall::VPE_CTRL;
    req.vpe_sel = vpe;
    req.op = static_cast<xfer_t>(op);
    req.arg = arg;
    if(vpe == KIF::SEL_VPE && op == KIF::Syscall::VCTRL_STOP)
        _sendgate.send(req_buf, 0);
    else
        send_receive_throw(req_buf);
}

int Syscalls::vpe_wait(const capsel_t *vpes, size_t count, event_t event, capsel_t *vpe) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::VPEWait>();
    req.opcode = KIF::Syscall::VPE_WAIT;
    req.vpe_count = count;
    req.event = event;
    for(size_t i = 0; i < count; ++i)
        req.sels[i] = vpes[i];

    auto reply = send_receive<KIF::Syscall::VPEWaitReply>(req_buf);

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
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::DeriveMem>();
    req.opcode = KIF::Syscall::DERIVE_MEM;
    req.vpe_sel = vpe;
    req.dst_sel = dst;
    req.src_sel = src;
    req.offset = offset;
    req.size = size;
    req.perms = static_cast<xfer_t>(perms);
    send_receive_throw(req_buf);
}

void Syscalls::derive_kmem(capsel_t kmem, capsel_t dst, size_t quota) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::DeriveKMem>();
    req.opcode = KIF::Syscall::DERIVE_KMEM;
    req.kmem_sel = kmem;
    req.dst_sel = dst;
    req.quota = quota;
    send_receive_throw(req_buf);
}

void Syscalls::derive_pe(capsel_t pe, capsel_t dst, uint eps, uint64_t time, uint64_t pts) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::DerivePE>();
    req.opcode = KIF::Syscall::DERIVE_PE;
    req.pe_sel = pe;
    req.dst_sel = dst;
    req.eps = eps;
    req.time = time;
    req.pts = pts;
    send_receive_throw(req_buf);
}

void Syscalls::derive_srv(capsel_t srv, const KIF::CapRngDesc &dst, uint sessions, event_t event) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::DeriveSrv>();
    req.opcode = KIF::Syscall::DERIVE_SRV;
    req.srv_sel = srv;
    req.dst_sel = dst.start();
    req.sessions = sessions;
    req.event = event;
    send_receive_throw(req_buf);
}

void Syscalls::get_sess(capsel_t srv, capsel_t vpe, capsel_t dst, word_t sid) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::GetSession>();
    req.opcode = KIF::Syscall::GET_SESS;
    req.srv_sel = srv;
    req.vpe_sel = vpe;
    req.dst_sel = dst;
    req.sid = sid;
    send_receive_throw(req_buf);
}

GlobAddr Syscalls::mgate_region(capsel_t mgate, size_t *size) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::MGateRegion>();
    req.opcode = KIF::Syscall::MGATE_REGION;
    req.mgate_sel = mgate;

    auto reply = send_receive<KIF::Syscall::MGateRegionReply>(req_buf);

    Errors::Code res = static_cast<Errors::Code>(reply.error());
    if(res != Errors::NONE)
        throw SyscallException(res, KIF::Syscall::MGATE_REGION);
    if(size)
        *size = reply->size;
    return GlobAddr(reply->global);
}

size_t Syscalls::kmem_quota(capsel_t kmem, size_t *total) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::KMemQuota>();
    req.opcode = KIF::Syscall::KMEM_QUOTA;
    req.kmem_sel = kmem;

    auto reply = send_receive<KIF::Syscall::KMemQuotaReply>(req_buf);

    Errors::Code res = static_cast<Errors::Code>(reply.error());
    if(res != Errors::NONE)
        throw SyscallException(res, KIF::Syscall::KMEM_QUOTA);
    if(total)
        *total = reply->total;
    return reply->amount;
}

void Syscalls::pe_quota(capsel_t pe,
                        uint *eps_total, uint *eps_left,
                        uint64_t *time_total, uint64_t *time_left,
                        size_t *pts_total, size_t *pts_left) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::PEQuota>();
    req.opcode = KIF::Syscall::PE_QUOTA;
    req.pe_sel = pe;

    auto reply = send_receive<KIF::Syscall::PEQuotaReply>(req_buf);

    Errors::Code res = static_cast<Errors::Code>(reply.error());
    if(res != Errors::NONE)
        throw SyscallException(res, KIF::Syscall::PE_QUOTA);
    if(eps_total)
        *eps_total = reply->eps_total;
    if(eps_left)
        *eps_left = reply->eps_left;
    if(time_total)
        *time_total = reply->time_total;
    if(time_left)
        *time_left = reply->time_left;
    if(pts_total)
        *pts_total = reply->pts_total;
    if(pts_left)
        *pts_left = reply->pts_left;
}

void Syscalls::pe_set_quota(capsel_t pe, uint64_t time, uint64_t pts) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::PESetQuota>();
    req.opcode = KIF::Syscall::PE_SET_QUOTA;
    req.pe_sel = pe;
    req.time = time;
    req.pts = pts;
    send_receive_throw(req_buf);
}

void Syscalls::sem_ctrl(capsel_t sel, KIF::Syscall::SemOp op) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::SemCtrl>();
    req.opcode = KIF::Syscall::SEM_CTRL;
    req.sem_sel = sel;
    req.op = op;
    send_receive_throw(req_buf);
}

void Syscalls::exchange(capsel_t vpe, const KIF::CapRngDesc &own, capsel_t other, bool obtain) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::Exchange>();
    req.opcode = KIF::Syscall::EXCHANGE;
    req.vpe_sel = vpe;
    own.to_raw(req.own_caps);
    req.other_sel = other;
    req.obtain = obtain;
    send_receive_throw(req_buf);
}

void Syscalls::exchange_sess(capsel_t vpe, capsel_t sess, const KIF::CapRngDesc &crd,
                             KIF::ExchangeArgs *args, bool obtain) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::ExchangeSess>();
    req.opcode = obtain ? KIF::Syscall::OBTAIN : KIF::Syscall::DELEGATE;
    req.vpe_sel = vpe;
    req.sess_sel = sess;
    crd.to_raw(req.caps);
    if(args)
        memcpy(&req.args, args, sizeof(*args));
    else
        req.args.bytes = 0;

    auto reply = send_receive<KIF::Syscall::ExchangeSessReply>(req_buf);

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
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::Revoke>();
    req.opcode = KIF::Syscall::REVOKE;
    req.vpe_sel = vpe;
    crd.to_raw(req.caps);
    req.own = own;
    send_receive_throw(req_buf);
}

void Syscalls::reset_stats() {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::ResetStats>();
    req.opcode = KIF::Syscall::RESET_STATS;
    send_receive_throw(req_buf);
}

void Syscalls::noop() {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::Noop>();
    req.opcode = KIF::Syscall::NOOP;
    send_receive_throw(req_buf);
}

}
