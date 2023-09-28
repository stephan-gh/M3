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

#pragma once

#include <base/GlobAddr.h>
#include <base/KIF.h>
#include <base/Quota.h>
#include <base/TileDesc.h>
#include <base/time/Duration.h>

#include <m3/Env.h>
#include <m3/com/GateStream.h>
#include <m3/com/SendGate.h>

namespace m3 {

class Env;
class EnvUserBackend;
class RecvGate;

class Syscalls {
    friend class Env;
    friend class EnvUserBackend;

    template<class T>
    struct SyscallReply {
        explicit SyscallReply(const TCU::Message *msg) : _msg(msg) {
        }
        ~SyscallReply() {
            RecvGate::syscall().ack_msg(_msg);
        }

        Errors::Code error() const {
            return static_cast<Errors::Code>(operator->()->error);
        }

        const T *operator->() const {
            return reinterpret_cast<const T *>(_msg->data);
        }

    private:
        const TCU::Message *_msg;
    };

    Syscalls() = delete;

public:
    static void create_srv(capsel_t dst, capsel_t rgate, const std::string_view &name,
                           label_t creator);
    static void create_sess(capsel_t dst, capsel_t srv, size_t crt, word_t ident, bool auto_close);
    static void create_mgate(capsel_t dst, capsel_t act, goff_t addr, size_t size, int perms);
    static void create_rgate(capsel_t dst, uint order, uint msgorder);
    static void create_sgate(capsel_t dst, capsel_t rgate, label_t label, uint credits);
    static std::pair<epid_t, actid_t> create_activity(capsel_t dst, const std::string_view &name,
                                                      capsel_t tile, capsel_t kmem);
    static void create_map(capsel_t dst, capsel_t act, capsel_t mgate, capsel_t first,
                           capsel_t pages, int perms);
    static void create_sem(capsel_t dst, uint value);
    static epid_t alloc_ep(capsel_t dst, capsel_t act, epid_t ep, uint replies);

    static void activate(capsel_t ep, capsel_t gate, capsel_t rbuf_mem, goff_t rbuf_off);
    static void activity_ctrl(capsel_t act, KIF::Syscall::ActivityOp op, xfer_t arg);
    static std::pair<Errors::Code, capsel_t> activity_wait(const capsel_t *acts, size_t count,
                                                           event_t event);
    static void derive_mem(capsel_t act, capsel_t dst, capsel_t src, goff_t offset, size_t size,
                           int perms);
    static void derive_kmem(capsel_t kmem, capsel_t dst, size_t quota);
    static void derive_tile(capsel_t tile, capsel_t dst, Option<uint> eps,
                            Option<TimeDuration> time, Option<size_t> pts);
    static void derive_srv(capsel_t srv, const KIF::CapRngDesc &dst, uint sessions, event_t event);
    static void get_sess(capsel_t srv, capsel_t act, capsel_t dst, word_t sid);
    static std::pair<GlobAddr, size_t> mgate_region(capsel_t mgate);
    static std::pair<uint, uint> rgate_buffer(capsel_t rgate);
    static Quota<size_t> kmem_quota(capsel_t kmem);
    static std::tuple<Quota<uint>, Quota<TimeDuration>, Quota<size_t>> tile_quota(capsel_t tile);
    static void tile_set_quota(capsel_t tile, TimeDuration time, size_t pts);
    static void tile_set_pmp(capsel_t tile, capsel_t mgate, epid_t epid, bool overwrite);
    static KIF::Syscall::TileMuxType tile_mux_info(capsel_t tile);
    static void tile_mem(capsel_t dst, capsel_t tile);
    static void tile_reset(capsel_t tile, capsel_t mux_mem);
    static void sem_ctrl(capsel_t sem, KIF::Syscall::SemOp);

    static void delegate(capsel_t act, capsel_t sess, const KIF::CapRngDesc &crd,
                         KIF::ExchangeArgs *args = nullptr);
    static void obtain(capsel_t act, capsel_t sess, const KIF::CapRngDesc &crd,
                       KIF::ExchangeArgs *args = nullptr);
    static void exchange(capsel_t act, const KIF::CapRngDesc &own, capsel_t other, bool obtain);
    static void revoke(capsel_t act, const KIF::CapRngDesc &crd, bool own = true);

    static void reset_stats();
    static void noop();

private:
    template<class T>
    static SyscallReply<T> send_receive(const MsgBuf &msg) noexcept;
    static Errors::Code send_receive_err(const MsgBuf &msg) noexcept;
    static void send_receive_throw(const MsgBuf &msg);
    static void exchange_sess(capsel_t act, capsel_t sess, const KIF::CapRngDesc &crd,
                              KIF::ExchangeArgs *args, bool obtain);

    static void reinit();

    static SendGate _sendgate;
};

}
