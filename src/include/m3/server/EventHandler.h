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

#include <base/col/SList.h>

#include <m3/com/GateStream.h>
#include <m3/server/Handler.h>
#include <m3/session/ServerSession.h>
#include <m3/tiles/Activity.h>

#include <memory>

namespace m3 {

template<class SESS>
class EventHandler;

class EventSessionData : public ServerSession, public SListItem {
    template<class SESS>
    friend class EventHandler;

public:
    explicit EventSessionData(size_t crt, capsel_t srv_sel) noexcept
        : ServerSession(crt, srv_sel),
          SListItem(),
          _sgate() {
    }

    std::unique_ptr<LazyGate<SendGate>> &gate() noexcept {
        return _sgate;
    }

protected:
    std::unique_ptr<LazyGate<SendGate>> _sgate;
};

template<class SESS = EventSessionData>
class EventHandler : public Handler<SESS> {
    template<class HDL>
    friend class Server;

public:
    explicit EventHandler() noexcept : Handler<SESS>(), _sessions() {
    }

    template<typename... Args>
    void broadcast(const Args &...args) {
        auto msg = create_vmsg(args...);
        for(auto &h : _sessions) {
            if(h.gate())
                send_msg(h.gate()->get(), msg.finish());
        }
    }

    SList<SESS> &sessions() noexcept {
        return _sessions;
    }

protected:
    virtual Errors::Code open(SESS **sess, size_t crt, capsel_t srv_sel,
                              const std::string_view &) override {
        *sess = new SESS(crt, srv_sel);
        _sessions.append(*sess);
        return Errors::SUCCESS;
    }

    virtual Errors::Code delegate(SESS *sess, size_t, CapExchange &xchg) override {
        if(sess->gate() || xchg.in_caps() != 1)
            return Errors::INV_ARGS;

        auto sel = SelSpace::get().alloc_sel();
        sess->_sgate = std::make_unique<LazyGate<SendGate>>(SendCap::bind(sel));
        xchg.out_caps(KIF::CapRngDesc(KIF::CapRngDesc::OBJ, sel));
        return Errors::SUCCESS;
    }

    virtual Errors::Code close(SESS *sess, size_t) override {
        _sessions.remove(sess);
        delete sess;
        return Errors::SUCCESS;
    }

private:
    SList<SESS> _sessions;
};

}
