/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2020 Nils Asmussen, Barkhausen Institut
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

#include <m3/com/RecvGate.h>
#include <m3/com/SendGate.h>
#include <m3/server/RequestHandler.h>
#include <m3/session/ServerSession.h>

#include <memory>

namespace m3 {

struct SimpleSession : public ServerSession {
    explicit SimpleSession(size_t crt, capsel_t srv_sel) noexcept
        : ServerSession(crt, srv_sel),
          scap() {
    }

    std::unique_ptr<SendCap> scap;
};

template<typename CLS, typename OP, size_t OPCNT, size_t MSG_SIZE = 128>
class SimpleRequestHandler : public RequestHandler<CLS, OP, OPCNT, SimpleSession> {
    static constexpr size_t BUF_SIZE = Server<SimpleRequestHandler>::MAX_SESSIONS * MSG_SIZE;

public:
    explicit SimpleRequestHandler(WorkLoop *wl)
        : RequestHandler<CLS, OP, OPCNT, SimpleSession>(),
          _rgate(RecvGate::create(nextlog2<BUF_SIZE>::val, nextlog2<MSG_SIZE>::val)) {
        using std::placeholders::_1;
        _rgate.start(wl, std::bind(&SimpleRequestHandler::handle_message, this, _1));
    }

    virtual Errors::Code open(SimpleSession **sess, size_t crt, capsel_t srv_sel,
                              const std::string_view &) override {
        *sess = new SimpleSession(crt, srv_sel);
        return Errors::SUCCESS;
    }

    virtual Errors::Code obtain(SimpleSession *sess, size_t, CapExchange &xchg) override {
        if(sess->scap || xchg.in_caps() != 1)
            return Errors::INV_ARGS;

        label_t label = ptr_to_label(sess);
        sess->scap = std::make_unique<SendCap>(
            SendCap::create(&_rgate, SendGateArgs().label(label).credits(1)));

        xchg.out_caps(KIF::CapRngDesc(KIF::CapRngDesc::OBJ, sess->scap->sel()));
        return Errors::SUCCESS;
    }

    virtual Errors::Code close(SimpleSession *sess, size_t) override {
        delete sess;
        _rgate.drop_msgs_with(ptr_to_label(sess));
        return Errors::SUCCESS;
    }

    virtual void shutdown() override {
        _rgate.stop();
    }

private:
    RecvGate _rgate;
};

}
