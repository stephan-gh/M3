/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

#include <m3/Exception.h>
#include <m3/Syscalls.h>
#include <m3/com/GateStream.h>

#include <utility>

namespace m3 {

INIT_PRIO_SYSCALLS SendGate
    Syscalls::_sendgate(KIF::INV_SEL, ObjCap::KEEP_CAP, &RecvGate::syscall(),
                        new EP(EP::bind(env()->first_std_ep + TCU::SYSC_SEP_OFF)));

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
    if(res != Errors::SUCCESS) {
        auto syscall = msg.get<KIF::DefaultRequest>();
        throw SyscallException(res, static_cast<KIF::Syscall::Operation>(syscall.opcode));
    }
}

void Syscalls::create_srv(capsel_t dst, capsel_t rgate, const std::string_view &name,
                          label_t creator) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::CreateSrv>();
    req.opcode = KIF::Syscall::CREATE_SRV;
    req.dst_sel = dst;
    req.rgate_sel = rgate;
    req.creator = creator;
    req.namelen = Math::min(name.length() + 1, sizeof(req.name));
    memcpy(req.name, name.data(), req.namelen - 1);
    req.name[req.namelen - 1] = '\0';
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

void Syscalls::create_mgate(capsel_t dst, capsel_t act, goff_t addr, size_t size, int perms) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::CreateMGate>();
    req.opcode = KIF::Syscall::CREATE_MGATE;
    req.dst_sel = dst;
    req.act_sel = act;
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

void Syscalls::create_map(capsel_t dst, capsel_t act, capsel_t mgate, capsel_t first,
                          capsel_t pages, int perms) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::CreateMap>();
    req.opcode = KIF::Syscall::CREATE_MAP;
    req.dst_sel = dst;
    req.act_sel = act;
    req.mgate_sel = mgate;
    req.first = first;
    req.pages = pages;
    req.perms = static_cast<xfer_t>(perms);
    send_receive_throw(req_buf);
}

std::pair<epid_t, actid_t> Syscalls::create_activity(capsel_t dst, const std::string_view &name,
                                                     capsel_t tile, capsel_t kmem) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::CreateActivity>();
    req.opcode = KIF::Syscall::CREATE_ACT;
    req.dst_sel = dst;
    req.tile_sel = tile;
    req.kmem_sel = kmem;
    req.namelen = Math::min(name.length() + 1, sizeof(req.name));
    memcpy(req.name, name.data(), req.namelen - 1);
    req.name[req.namelen - 1] = '\0';

    auto reply = send_receive<KIF::Syscall::CreateActivityReply>(req_buf);

    Errors::Code res = static_cast<Errors::Code>(reply.error());
    if(res != Errors::SUCCESS)
        throw SyscallException(res, KIF::Syscall::CREATE_ACT);
    return std::make_pair(reply->eps_start, reply->id);
}

void Syscalls::create_sem(capsel_t dst, uint value) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::CreateSem>();
    req.opcode = KIF::Syscall::CREATE_SEM;
    req.dst_sel = dst;
    req.value = value;
    send_receive_throw(req_buf);
}

epid_t Syscalls::alloc_ep(capsel_t dst, capsel_t act, epid_t ep, uint replies) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::AllocEP>();
    req.opcode = KIF::Syscall::ALLOC_EPS;
    req.dst_sel = dst;
    req.act_sel = act;
    req.epid = ep;
    req.replies = replies;

    auto reply = send_receive<KIF::Syscall::AllocEPReply>(req_buf);

    Errors::Code res = static_cast<Errors::Code>(reply.error());
    if(res != Errors::SUCCESS)
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

void Syscalls::activity_ctrl(capsel_t act, KIF::Syscall::ActivityOp op, xfer_t arg) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::ActivityCtrl>();
    req.opcode = KIF::Syscall::ACT_CTRL;
    req.act_sel = act;
    req.op = static_cast<xfer_t>(op);
    req.arg = arg;
    if(act == KIF::SEL_ACT && op == KIF::Syscall::VCTRL_STOP)
        _sendgate.send(req_buf, 0);
    else
        send_receive_throw(req_buf);
}

std::pair<Errors::Code, capsel_t> Syscalls::activity_wait(const capsel_t *acts, size_t count,
                                                          event_t event) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::ActivityWait>();
    req.opcode = KIF::Syscall::ACT_WAIT;
    req.act_count = count;
    req.event = event;
    for(size_t i = 0; i < count; ++i)
        req.sels[i] = acts[i];

    auto reply = send_receive<KIF::Syscall::ActivityWaitReply>(req_buf);

    Errors::Code exitcode = Errors::UNSPECIFIED;
    capsel_t act = KIF::INV_SEL;
    Errors::Code res = static_cast<Errors::Code>(reply.error());
    if(res == Errors::SUCCESS && event == 0) {
        act = reply->act_sel;
        exitcode = static_cast<Errors::Code>(reply->exitcode);
    }

    if(res != Errors::SUCCESS)
        throw SyscallException(res, KIF::Syscall::ACT_WAIT);
    return std::make_pair(exitcode, act);
}

void Syscalls::derive_mem(capsel_t act, capsel_t dst, capsel_t src, goff_t offset, size_t size,
                          int perms) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::DeriveMem>();
    req.opcode = KIF::Syscall::DERIVE_MEM;
    req.act_sel = act;
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

void Syscalls::derive_tile(capsel_t tile, capsel_t dst, Option<uint> eps, Option<TimeDuration> time,
                           Option<size_t> pts) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::DeriveTile>();
    req.opcode = KIF::Syscall::DERIVE_TILE;
    req.tile_sel = tile;
    req.dst_sel = dst;
    req.eps = eps.unwrap_or(static_cast<uint>(-1));
    if(auto duration = time)
        req.time = duration.unwrap().as_nanos();
    else
        req.time = static_cast<uint64_t>(-1);
    req.pts = pts.unwrap_or(static_cast<size_t>(-1));
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

void Syscalls::get_sess(capsel_t srv, capsel_t act, capsel_t dst, word_t sid) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::GetSession>();
    req.opcode = KIF::Syscall::GET_SESS;
    req.srv_sel = srv;
    req.act_sel = act;
    req.dst_sel = dst;
    req.sid = sid;
    send_receive_throw(req_buf);
}

std::pair<GlobAddr, size_t> Syscalls::mgate_region(capsel_t mgate) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::MGateRegion>();
    req.opcode = KIF::Syscall::MGATE_REGION;
    req.mgate_sel = mgate;

    auto reply = send_receive<KIF::Syscall::MGateRegionReply>(req_buf);

    Errors::Code res = static_cast<Errors::Code>(reply.error());
    if(res != Errors::SUCCESS)
        throw SyscallException(res, KIF::Syscall::MGATE_REGION);
    return std::make_pair(GlobAddr(reply->global), reply->size);
}

std::pair<uint, uint> Syscalls::rgate_buffer(capsel_t rgate) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::RGateBuffer>();
    req.opcode = KIF::Syscall::RGATE_BUFFER;
    req.rgate_sel = rgate;

    auto reply = send_receive<KIF::Syscall::RGateBufferReply>(req_buf);

    Errors::Code res = static_cast<Errors::Code>(reply.error());
    if(res != Errors::SUCCESS)
        throw SyscallException(res, KIF::Syscall::RGATE_BUFFER);
    return std::make_pair(reply->order, reply->msg_order);
}

Quota<size_t> Syscalls::kmem_quota(capsel_t kmem) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::KMemQuota>();
    req.opcode = KIF::Syscall::KMEM_QUOTA;
    req.kmem_sel = kmem;

    auto reply = send_receive<KIF::Syscall::KMemQuotaReply>(req_buf);

    Errors::Code res = static_cast<Errors::Code>(reply.error());
    if(res != Errors::SUCCESS)
        throw SyscallException(res, KIF::Syscall::KMEM_QUOTA);
    return Quota<size_t>(reply->id, reply->total, reply->left);
}

std::tuple<Quota<uint>, Quota<TimeDuration>, Quota<size_t>> Syscalls::tile_quota(capsel_t tile) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::TileQuota>();
    req.opcode = KIF::Syscall::TILE_QUOTA;
    req.tile_sel = tile;

    auto reply = send_receive<KIF::Syscall::TileQuotaReply>(req_buf);

    Errors::Code res = static_cast<Errors::Code>(reply.error());
    if(res != Errors::SUCCESS)
        throw SyscallException(res, KIF::Syscall::TILE_QUOTA);

    return std::make_tuple(Quota<uint>(reply->eps_id, reply->eps_total, reply->eps_left),
                           Quota<TimeDuration>(reply->time_id,
                                               TimeDuration::from_nanos(reply->time_total),
                                               TimeDuration::from_nanos(reply->time_left)),
                           Quota<size_t>(reply->pts_id, reply->pts_total, reply->pts_left));
}

void Syscalls::tile_set_quota(capsel_t tile, TimeDuration time, size_t pts) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::TileSetQuota>();
    req.opcode = KIF::Syscall::TILE_SET_QUOTA;
    req.tile_sel = tile;
    req.time = time.as_nanos();
    req.pts = pts;
    send_receive_throw(req_buf);
}

void Syscalls::tile_set_pmp(capsel_t tile, capsel_t mgate, epid_t epid, bool overwrite) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::TileSetPMP>();
    req.opcode = KIF::Syscall::TILE_SET_PMP;
    req.tile_sel = tile;
    req.mgate_sel = mgate;
    req.epid = epid;
    req.overwrite = overwrite;
    send_receive_throw(req_buf);
}

KIF::Syscall::MuxType Syscalls::tile_mux_info(capsel_t tile) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::TileMuxInfo>();
    req.opcode = KIF::Syscall::TILE_MUX_INFO;
    req.tile_sel = tile;

    auto reply = send_receive<KIF::Syscall::TileMuxInfoReply>(req_buf);

    Errors::Code res = static_cast<Errors::Code>(reply.error());
    if(res != Errors::SUCCESS)
        throw SyscallException(res, static_cast<KIF::Syscall::Operation>(req.opcode));
    return static_cast<KIF::Syscall::MuxType>(reply->type);
}

void Syscalls::tile_mem(capsel_t dst, capsel_t tile) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::TileMem>();
    req.opcode = KIF::Syscall::TILE_MEM;
    req.dst_sel = dst;
    req.tile_sel = tile;
    send_receive_throw(req_buf);
}

void Syscalls::tile_reset(capsel_t tile, capsel_t mux_mem) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::TileReset>();
    req.opcode = KIF::Syscall::TILE_RESET;
    req.tile_sel = tile;
    req.mux_mem_sel = mux_mem;
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

void Syscalls::exchange(capsel_t act, const KIF::CapRngDesc &own, capsel_t other, bool obtain) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::Exchange>();
    req.opcode = KIF::Syscall::EXCHANGE;
    req.act_sel = act;
    own.to_raw(req.own_caps);
    req.other_sel = other;
    req.obtain = obtain;
    send_receive_throw(req_buf);
}

void Syscalls::exchange_sess(capsel_t act, capsel_t sess, const KIF::CapRngDesc &crd,
                             KIF::ExchangeArgs *args, bool obtain) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::ExchangeSess>();
    req.opcode = KIF::Syscall::EXCHANGE_SESS;
    req.act_sel = act;
    req.sess_sel = sess;
    req.obtain = obtain;
    crd.to_raw(req.caps);
    if(args)
        memcpy(&req.args, args, sizeof(*args));
    else
        req.args.bytes = 0;

    auto reply = send_receive<KIF::Syscall::ExchangeSessReply>(req_buf);

    Errors::Code res = static_cast<Errors::Code>(reply.error());
    if(res != Errors::SUCCESS)
        throw SyscallException(res, static_cast<KIF::Syscall::Operation>(req.opcode));
    if(args)
        memcpy(args, &reply->args, sizeof(*args));
}

void Syscalls::delegate(capsel_t act, capsel_t sess, const KIF::CapRngDesc &crd,
                        KIF::ExchangeArgs *args) {
    exchange_sess(act, sess, crd, args, false);
}

void Syscalls::obtain(capsel_t act, capsel_t sess, const KIF::CapRngDesc &crd,
                      KIF::ExchangeArgs *args) {
    exchange_sess(act, sess, crd, args, true);
}

void Syscalls::revoke(capsel_t act, const KIF::CapRngDesc &crd, bool own) {
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::Revoke>();
    req.opcode = KIF::Syscall::REVOKE;
    req.act_sel = act;
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
