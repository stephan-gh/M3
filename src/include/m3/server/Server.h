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

#include <base/Errors.h>
#include <base/KIF.h>
#include <base/Log.h>

#include <m3/Syscalls.h>
#include <m3/com/RecvGate.h>
#include <m3/server/Handler.h>
#include <m3/session/ResMng.h>
#include <m3/stream/Standard.h>
#include <m3/tiles/Activity.h>

#include <memory>

namespace m3 {

template<class HDL>
class Server : public ObjCap {
    using handler_func = void (Server::*)(GateIStream &is);

    static constexpr size_t MAX_CREATORS = 3;
    static constexpr size_t MSG_SIZE = 256;
    static constexpr size_t BUF_SIZE = MSG_SIZE * (MAX_CREATORS + 1);

    struct Creator {
        explicit Creator(SendCap &&_scap, size_t _sessions)
            : scap(std::move(_scap)),
              sessions(_sessions) {
        }

        SendCap scap;
        size_t sessions;
    };

public:
    static constexpr size_t MAX_SESSIONS =
        Math::min(static_cast<size_t>(MAX_ACTS), TCU::MAX_RB_SIZE);

    explicit Server(const std::string_view &name, WorkLoop *wl, std::unique_ptr<HDL> &&handler)
        : ObjCap(SERVICE, SelSpace::get().alloc_sel()),
          _handler(std::move(handler)),
          _ctrl_handler(),
          _creators(),
          _rgate(RecvGate::create(nextlog2<BUF_SIZE>::val, nextlog2<MSG_SIZE>::val)) {
        init(wl);

        LOG(LogFlags::LibServ, "create(name={})"_cf, name);
        size_t crt = add_creator(MAX_SESSIONS);
        Syscalls::create_srv(sel(), _rgate.sel(), name, crt);
        Activity::own().resmng()->reg_service(sel(), _creators[crt]->scap.sel(), name,
                                              MAX_SESSIONS);
    }

    ~Server() {
        try {
            Activity::own().resmng()->unreg_service(sel());
        }
        catch(...) {
            // ignore
        }
    }

    void shutdown() {
        _handler->shutdown();
        _rgate.stop();
    }

    std::unique_ptr<HDL> &handler() {
        return _handler;
    }

private:
    void init(WorkLoop *wl) {
        using std::placeholders::_1;
        _rgate.start(wl, std::bind(&Server::handle_message, this, _1));

        _ctrl_handler[KIF::Service::OPEN] = &Server::handle_open;
        _ctrl_handler[KIF::Service::DERIVE_CRT] = &Server::handle_derive_crt;
        _ctrl_handler[KIF::Service::OBTAIN] = &Server::handle_obtain;
        _ctrl_handler[KIF::Service::DELEGATE] = &Server::handle_delegate;
        _ctrl_handler[KIF::Service::CLOSE] = &Server::handle_close;
        _ctrl_handler[KIF::Service::SHUTDOWN] = &Server::handle_shutdown;
    }

    void handle_message(GateIStream &is) {
        auto *req = reinterpret_cast<const KIF::DefaultRequest *>(is.message().data);
        KIF::Service::Operation op = static_cast<KIF::Service::Operation>(req->opcode);

        if(static_cast<size_t>(op) < ARRAY_SIZE(_ctrl_handler)) {
            try {
                (this->*_ctrl_handler[op])(is);
            }
            catch(const Exception &e) {
                LOG(LogFlags::Error, "exception during service request: {}"_cf, e.what());
                try {
                    reply_error(is, e.code());
                }
                catch(...) {
                    // ignore
                }
            }
            return;
        }

        try {
            reply_error(is, Errors::INV_ARGS);
        }
        catch(...) {
            // ignore
        }
    }

    void handle_open(GateIStream &is) {
        auto *req = reinterpret_cast<const KIF::Service::Open *>(is.message().data);

        // check and reduce session quota
        label_t crt = is.message().label;
        if(crt >= MAX_CREATORS || !_creators[crt] || _creators[crt]->sessions == 0) {
            reply_error(is, Errors::NO_SPACE);
            return;
        }
        _creators[crt]->sessions--;

        MsgBuf reply_buf;
        auto &reply = reply_buf.cast<KIF::Service::OpenReply>();

        typename HDL::session_type *sess = nullptr;
        std::string_view arg(req->arg,
                             Math::min(static_cast<size_t>(req->arglen - 1), sizeof(req->arg)));
        reply.error = _handler->open(&sess, crt, sel(), arg);
        if(sess)
            LOG(LogFlags::LibServ, "{:#x}: open()"_cf, (word_t)sess);

        reply.sess = sess ? sess->sel() : KIF::INV_SEL;
        reply.ident = reinterpret_cast<uintptr_t>(sess);
        is.reply(reply_buf);
    }

    void handle_derive_crt(GateIStream &is) {
        auto *req = reinterpret_cast<const KIF::Service::DeriveCreator *>(is.message().data);

        size_t crt = is.label<size_t>();
        size_t sessions = req->sessions;
        assert(crt < MAX_CREATORS && _creators[crt] != nullptr);

        LOG(LogFlags::LibServ, "derive_crt(creator={}, sessions={})"_cf, crt, sessions);

        MsgBuf reply_buf;
        auto &reply = reply_buf.cast<KIF::Service::DeriveCreatorReply>();
        reply.error = 0;

        if(!_creators[crt] || sessions > _creators[crt]->sessions)
            reply.error = Errors::NO_SPACE;
        else {
            size_t ncrt = add_creator(sessions);
            _creators[crt]->sessions -= sessions;
            reply.sgate_sel = _creators[ncrt]->scap.sel();
            reply.creator = ncrt;
        }

        is.reply(reply_buf);
    }

    void handle_obtain(GateIStream &is) {
        auto *req = reinterpret_cast<const KIF::Service::Exchange *>(is.message().data);
        // TODO isolate creators from each other
        label_t crt = is.message().label;

        LOG(LogFlags::LibServ, "{:#x}: obtain(caps={}:{}, args={})"_cf, (word_t)req->sess,
            req->data.caps[0], req->data.caps[1], req->data.args.bytes);

        MsgBuf reply_buf;
        auto &reply = reply_buf.cast<KIF::Service::ExchangeReply>();
        CapExchange xchg(req->data, reply.data);

        typename HDL::session_type *sess =
            reinterpret_cast<typename HDL::session_type *>(req->sess);
        reply.error = _handler->obtain(sess, crt, xchg);

        reply.data.args.bytes = xchg.out_args().total();
        is.reply(reply_buf);
    }

    void handle_delegate(GateIStream &is) {
        auto *req = reinterpret_cast<const KIF::Service::Exchange *>(is.message().data);
        label_t crt = is.message().label;

        LOG(LogFlags::LibServ, "{:#x}: delegate(caps={}:{}, args={})"_cf, (word_t)req->sess,
            req->data.caps[0], req->data.caps[1], req->data.args.bytes);

        MsgBuf reply_buf;
        auto &reply = reply_buf.cast<KIF::Service::ExchangeReply>();
        CapExchange xchg(req->data, reply.data);

        typename HDL::session_type *sess =
            reinterpret_cast<typename HDL::session_type *>(req->sess);
        reply.error = _handler->delegate(sess, crt, xchg);

        reply.data.args.bytes = xchg.out_args().total();
        is.reply(reply_buf);
    }

    void handle_close(GateIStream &is) {
        auto *req = reinterpret_cast<const KIF::Service::Close *>(is.message().data);

        // increase session quota
        label_t crt = is.message().label;
        assert(crt < MAX_CREATORS && _creators[crt] != nullptr);
        _creators[crt]->sessions++;

        LOG(LogFlags::LibServ, "{:#x}: close()"_cf, (word_t)req->sess);

        typename HDL::session_type *sess =
            reinterpret_cast<typename HDL::session_type *>(req->sess);
        Errors::Code res = _handler->close(sess, crt);

        reply_error(is, res);
    }

    void handle_shutdown(GateIStream &is) {
        LOG(LogFlags::LibServ, "shutdown()"_cf, 0);

        shutdown();

        reply_error(is, Errors::SUCCESS);
    }

    size_t add_creator(size_t sessions) {
        for(size_t i = 0; i < MAX_CREATORS; ++i) {
            if(_creators[i] == nullptr) {
                _creators[i] = std::make_unique<Creator>(
                    SendCap::create(&_rgate, SendGateArgs().credits(1).label(i)), sessions);
                return i;
            }
        }
        return MAX_CREATORS;
    }

protected:
    std::unique_ptr<HDL> _handler;
    handler_func _ctrl_handler[KIF::Service::SHUTDOWN + 1];
    std::unique_ptr<Creator> _creators[MAX_CREATORS];
    RecvGate _rgate;
};

}
