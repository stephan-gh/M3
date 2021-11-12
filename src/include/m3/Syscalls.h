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

#pragma once

#include <base/util/String.h>
#include <base/Env.h>
#include <base/GlobAddr.h>
#include <base/KIF.h>
#include <base/PEDesc.h>
#include <base/Quota.h>

#include <m3/com/SendGate.h>
#include <m3/com/GateStream.h>

namespace m3 {

class Env;
class EnvUserBackend;
class RecvGate;

class Syscalls {
    friend class Env;
    friend class EnvUserBackend;

    template<class T>
    struct SyscallReply {
        explicit SyscallReply(const TCU::Message *msg)
            : _msg(msg) {
        }
        ~SyscallReply() {
            RecvGate::syscall().ack_msg(_msg);
        }

        Errors::Code error() const {
            return static_cast<Errors::Code>(operator->()->error);
        }

        const T *operator->() const {
            return reinterpret_cast<const T*>(_msg->data);
        }

    private:
        const TCU::Message *_msg;
    };

    Syscalls() = delete;

public:
    static void create_srv(capsel_t dst, capsel_t rgate, const String &name, label_t creator);
    static void create_sess(capsel_t dst, capsel_t srv, size_t crt, word_t ident, bool auto_close);
    static void create_mgate(capsel_t dst, capsel_t vpe, goff_t addr, size_t size, int perms);
    static void create_rgate(capsel_t dst, uint order, uint msgorder);
    static void create_sgate(capsel_t dst, capsel_t rgate, label_t label, uint credits);
    static epid_t create_vpe(capsel_t dst, capsel_t pg_sg, capsel_t pg_rg,
                             const String &name, capsel_t pe, capsel_t kmem, vpeid_t *id);
    static void create_map(capsel_t dst, capsel_t vpe, capsel_t mgate, capsel_t first,
                           capsel_t pages, int perms);
    static void create_sem(capsel_t dst, uint value);
    static epid_t alloc_ep(capsel_t dst, capsel_t vpe, epid_t ep, uint replies);

    static void activate(capsel_t ep, capsel_t gate, capsel_t rbuf_mem, goff_t rbuf_off);
    static void set_pmp(capsel_t pe, capsel_t mgate, epid_t epid);
    static void vpe_ctrl(capsel_t vpe, KIF::Syscall::VPEOp op, xfer_t arg);
    static int vpe_wait(const capsel_t *vpes, size_t count, event_t event, capsel_t *vpe);
    static void derive_mem(capsel_t vpe, capsel_t dst, capsel_t src, goff_t offset,
                           size_t size, int perms);
    static void derive_kmem(capsel_t kmem, capsel_t dst, size_t quota);
    static void derive_pe(capsel_t pe, capsel_t dst,
                          uint eps = static_cast<uint>(-1),
                          uint64_t time = static_cast<uint64_t>(-1),
                          uint64_t pts = static_cast<uint64_t>(-1));
    static void derive_srv(capsel_t srv, const KIF::CapRngDesc &dst, uint sessions, event_t event);
    static void get_sess(capsel_t srv, capsel_t vpe, capsel_t dst, word_t sid);
    static GlobAddr mgate_region(capsel_t mgate, size_t *size);
    static Quota<size_t> kmem_quota(capsel_t kmem);
    static void pe_quota(capsel_t pe, Quota<uint> *eps, Quota<uint64_t> *time, Quota<size_t> *pts);
    static void pe_set_quota(capsel_t pe, uint64_t time, uint64_t pts);
    static void sem_ctrl(capsel_t sem, KIF::Syscall::SemOp);

    static void delegate(capsel_t vpe, capsel_t sess, const KIF::CapRngDesc &crd,
                         KIF::ExchangeArgs *args = nullptr);
    static void obtain(capsel_t vpe, capsel_t sess, const KIF::CapRngDesc &crd,
                       KIF::ExchangeArgs *args = nullptr);
    static void exchange(capsel_t vpe, const KIF::CapRngDesc &own, capsel_t other, bool obtain);
    static void revoke(capsel_t vpe, const KIF::CapRngDesc &crd, bool own = true);

    static void reset_stats();
    static void noop();

private:
    template<class T>
    static SyscallReply<T> send_receive(const MsgBuf &msg) noexcept;
    static Errors::Code send_receive_err(const MsgBuf &msg) noexcept;
    static void send_receive_throw(const MsgBuf &msg);
    static void exchange_sess(capsel_t vpe, capsel_t sess, const KIF::CapRngDesc &crd,
                              KIF::ExchangeArgs *args, bool obtain);

    static void reinit();

    static SendGate _sendgate;
};

}
