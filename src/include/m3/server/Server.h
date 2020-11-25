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

#include <base/log/Lib.h>
#include <base/Errors.h>
#include <base/KIF.h>

#include <m3/com/RecvGate.h>
#include <m3/server/Handler.h>
#include <m3/session/ResMng.h>
#include <m3/stream/Standard.h>
#include <m3/Syscalls.h>
#include <m3/pes/VPE.h>

#include <memory>

namespace m3 {

template<class HDL>
class Server : public ObjCap {
    using handler_func = void (Server::*)(GateIStream &is);

    static constexpr size_t MAX_CREATORS = 3;
    static constexpr size_t MSG_SIZE = 256;
    static constexpr size_t BUF_SIZE = MSG_SIZE * (MAX_CREATORS + 1);

    struct Creator {
        explicit Creator(SendGate &&_sgate, size_t _sessions)
            : sgate(std::move(_sgate)),
              sessions(_sessions) {
        }

        SendGate sgate;
        size_t sessions;
    };

public:
    static constexpr size_t MAX_SESSIONS = Math::min(MAX_VPES, 32);

    explicit Server(const String &name, WorkLoop *wl, std::unique_ptr<HDL> &&handler)
        : ObjCap(SERVICE, VPE::self().alloc_sel()),
          _handler(std::move(handler)),
          _ctrl_handler(),
          _creators(),
          _rgate(RecvGate::create(nextlog2<BUF_SIZE>::val, nextlog2<MSG_SIZE>::val)) {

        init(wl);

        LLOG(SERV, "create(name=" << name << ")");
        size_t crt = add_creator(MAX_SESSIONS);
        Syscalls::create_srv(sel(), _rgate.sel(), name, crt);
        VPE::self().resmng()->reg_service(sel(), _creators[crt]->sgate.sel(), name, MAX_SESSIONS);
    }

    ~Server() {
        try {
            VPE::self().resmng()->unreg_service(sel(), false);
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
        auto *req = reinterpret_cast<const KIF::DefaultRequest*>(is.message().data);
        KIF::Service::Operation op = static_cast<KIF::Service::Operation>(req->opcode);

        if(static_cast<size_t>(op) < ARRAY_SIZE(_ctrl_handler)) {
            try {
                (this->*_ctrl_handler[op])(is);
            }
            catch(const Exception &e) {
                cerr << "exception during service request: " << e.what() << "\n";
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
        auto *req = reinterpret_cast<const KIF::Service::Open*>(is.message().data);

        // check and reduce session quota
        label_t crt = is.message().label;
        if(crt >= MAX_CREATORS || !_creators[crt] || _creators[crt]->sessions == 0) {
            reply_error(is, Errors::NO_SPACE);
            return;
        }
        _creators[crt]->sessions--;

        KIF::Service::OpenReply reply;

        typename HDL::session_type *sess = nullptr;
        StringRef arg(req->arg, Math::min(static_cast<size_t>(req->arglen - 1), sizeof(req->arg)));
        reply.error = _handler->open(&sess, crt, sel(), arg);
        if(sess)
            LLOG(SERV, fmt((word_t)sess, "#x") << ": open()");

        reply.sess = sess ? sess->sel() : KIF::INV_SEL;
        reply.ident = reinterpret_cast<uintptr_t>(sess);
        is.reply(&reply, sizeof(reply));
    }

    void handle_derive_crt(GateIStream &is) {
        auto *req = reinterpret_cast<const KIF::Service::DeriveCreator*>(is.message().data);

        size_t crt = is.label<size_t>();
        size_t sessions = req->sessions;
        assert(crt < MAX_CREATORS && _creators[crt] != nullptr);

        LLOG(SERV, "derive_crt(creator=" << crt << ", sessions=" << sessions << ")");

        KIF::Service::DeriveCreatorReply reply;
        reply.error = 0;

        if(!_creators[crt] || sessions > _creators[crt]->sessions)
            reply.error = Errors::NO_SPACE;
        else {
            size_t ncrt = add_creator(sessions);
            _creators[crt]->sessions -= sessions;
            reply.sgate_sel = _creators[ncrt]->sgate.sel();
            reply.creator = ncrt;
        }

        is.reply(&reply, sizeof(reply));
    }

    void handle_obtain(GateIStream &is) {
        auto *req = reinterpret_cast<const KIF::Service::Exchange*>(is.message().data);
        // TODO isolate creators from each other
        label_t crt = is.message().label;

        LLOG(SERV, fmt((word_t)req->sess, "#x") << ": obtain(caps="
            << req->data.caps << ", args=" << req->data.args.bytes << ")");

        KIF::Service::ExchangeReply reply;
        CapExchange xchg(req->data, reply.data);

        typename HDL::session_type *sess = reinterpret_cast<typename HDL::session_type*>(req->sess);
        reply.error = _handler->obtain(sess, crt, xchg);

        reply.data.args.bytes = xchg.out_args().total();
        is.reply(&reply, sizeof(reply));
    }

    void handle_delegate(GateIStream &is) {
        auto *req = reinterpret_cast<const KIF::Service::Exchange*>(is.message().data);
        label_t crt = is.message().label;

        LLOG(SERV, fmt((word_t)req->sess, "#x") << ": delegate(caps="
            << req->data.caps << ", args=" << req->data.args.bytes << ")");

        KIF::Service::ExchangeReply reply;
        CapExchange xchg(req->data, reply.data);

        typename HDL::session_type *sess = reinterpret_cast<typename HDL::session_type*>(req->sess);
        reply.error = _handler->delegate(sess, crt, xchg);

        reply.data.args.bytes = xchg.out_args().total();
        is.reply(&reply, sizeof(reply));
    }

    void handle_close(GateIStream &is) {
        auto *req = reinterpret_cast<const KIF::Service::Close*>(is.message().data);

        // increase session quota
        label_t crt = is.message().label;
        assert(crt < MAX_CREATORS && _creators[crt] != nullptr);
        _creators[crt]->sessions++;

        LLOG(SERV, fmt((word_t)req->sess, "#x") << ": close()");

        typename HDL::session_type *sess = reinterpret_cast<typename HDL::session_type*>(req->sess);
        Errors::Code res = _handler->close(sess, crt);

        reply_error(is, res);
    }

    void handle_shutdown(GateIStream &is) {
        LLOG(SERV, "shutdown()");

        shutdown();

        reply_error(is, Errors::NONE);
    }

    size_t add_creator(size_t sessions) {
        for(size_t i = 0; i < MAX_CREATORS; ++i) {
            if(_creators[i] == nullptr) {
                _creators[i] = std::make_unique<Creator>(
                    SendGate::create(&_rgate, SendGateArgs().credits(1).label(i)),
                    sessions
                );
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
