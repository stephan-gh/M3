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
#include <base/KIF.h>
#include <base/PEDesc.h>

#include <m3/com/SendGate.h>
#include <m3/com/GateStream.h>

namespace m3 {

class Env;
class RecvGate;

class Syscalls {
    friend class Env;

    template<class T>
    struct SyscallReply {
        explicit SyscallReply(Errors::Code res, const TCU::Message *msg)
            : _res(res),
              _msg(msg) {
        }
        ~SyscallReply() {
            TCUIf::ack_msg(RecvGate::syscall(), _msg);
        }

        Errors::Code error() const {
            if(_res != Errors::NONE)
                return _res;
            return static_cast<Errors::Code>(operator->()->error);
        }

        const T *operator->() const {
            return reinterpret_cast<const T*>(_msg->data);
        }

    private:
        Errors::Code _res;
        const TCU::Message *_msg;
    };

    Syscalls() = delete;

public:
    static void create_srv(capsel_t dst, capsel_t vpe, capsel_t rgate, const String &name);
    static void create_sess(capsel_t dst, capsel_t srv, word_t ident);
    static void create_rgate(capsel_t dst, uint order, uint msgorder);
    static void create_sgate(capsel_t dst, capsel_t rgate, label_t label, uint credits);
    static epid_t create_vpe(const KIF::CapRngDesc &dst, capsel_t pg_sg, capsel_t pg_rg,
                             const String &name, capsel_t pe, capsel_t kmem);
    static void create_map(capsel_t dst, capsel_t vpe, capsel_t mgate, capsel_t first,
                           capsel_t pages, int perms);
    static void create_sem(capsel_t dst, uint value);
    static epid_t alloc_ep(capsel_t dst, capsel_t vpe, epid_t ep, uint replies);

    static void activate(capsel_t ep, capsel_t gate, goff_t addr);
    static void vpe_ctrl(capsel_t vpe, KIF::Syscall::VPEOp op, xfer_t arg);
    static int vpe_wait(const capsel_t *vpes, size_t count, event_t event, capsel_t *vpe);
    static void derive_mem(capsel_t vpe, capsel_t dst, capsel_t src, goff_t offset,
                           size_t size, int perms);
    static void derive_kmem(capsel_t kmem, capsel_t dst, size_t quota);
    static void derive_pe(capsel_t pe, capsel_t dst, uint eps);
    static size_t kmem_quota(capsel_t kmem);
    static uint pe_quota(capsel_t pe);
    static void sem_ctrl(capsel_t sem, KIF::Syscall::SemOp);

    static void delegate(capsel_t vpe, capsel_t sess, const KIF::CapRngDesc &crd,
                         KIF::ExchangeArgs *args = nullptr);
    static void obtain(capsel_t vpe, capsel_t sess, const KIF::CapRngDesc &crd,
                       KIF::ExchangeArgs *args = nullptr);
    static void exchange(capsel_t vpe, const KIF::CapRngDesc &own, capsel_t other, bool obtain);
    static void revoke(capsel_t vpe, const KIF::CapRngDesc &crd, bool own = true);

    static void noop();

private:
    template<class T>
    static SyscallReply<T> send_receive(const void *msg, size_t size) noexcept;
    static Errors::Code send_receive_err(const void *msg, size_t size) noexcept;
    static void send_receive_throw(const void *msg, size_t size);
    static void exchange_sess(capsel_t vpe, capsel_t sess, const KIF::CapRngDesc &crd,
                              KIF::ExchangeArgs *args, bool obtain);

    static SendGate _sendgate;
};

}
