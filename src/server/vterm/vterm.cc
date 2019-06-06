/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/log/Services.h>
#include <base/util/BitField.h>

#include <m3/com/MemGate.h>
#include <m3/server/Server.h>
#include <m3/server/RequestHandler.h>
#include <m3/session/ServerSession.h>
#include <m3/vfs/GenericFile.h>

using namespace m3;

static constexpr size_t MSG_SIZE    = 64;
static constexpr size_t BUF_SIZE    = 256;
static constexpr size_t MAX_CLIENTS = 32;

struct VTermSession;
class ChannelSession;
class VTermHandler;

using base_class = RequestHandler<
    VTermHandler, GenericFile::Operation, GenericFile::Operation::COUNT, VTermSession
>;

static Server<VTermHandler> *srv;

struct VTermSession : public ServerSession {
    enum Type {
        META,
        CHAN,
    };

    explicit VTermSession(capsel_t srv_sel, capsel_t caps)
        : ServerSession(srv_sel, caps) {
    }
    virtual ~VTermSession() {
    }

    virtual Type type() const = 0;

    virtual void next_in(m3::GateIStream &is) {
        reply_error(is, m3::Errors::NOT_SUP);
    }
    virtual void next_out(m3::GateIStream &is) {
        reply_error(is, m3::Errors::NOT_SUP);
    }
    virtual void commit(m3::GateIStream &is, size_t) {
        reply_error(is, m3::Errors::NOT_SUP);
    }
};

class MetaSession : public VTermSession {
public:
    explicit MetaSession(capsel_t srv_sel, capsel_t caps = ObjCap::INVALID)
        : VTermSession(srv_sel, caps) {
    }

    ChannelSession *create_chan(bool write);

    virtual Type type() const {
        return META;
    }
};

class ChannelSession : public VTermSession {
public:
    explicit ChannelSession(uint id, RecvGate &rgate, MemGate &mem,
                            capsel_t srv_sel, capsel_t caps, bool _writing)
        : VTermSession(srv_sel, caps),
          id(id),
          active(false),
          writing(_writing),
          ep(ObjCap::INVALID),
          sgate(SendGate::create(&rgate, reinterpret_cast<label_t>(this), MSG_SIZE, nullptr, caps + 1)),
          mem(mem.derive(id * BUF_SIZE, BUF_SIZE, MemGate::RW)),
          pos(),
          len() {
    }
    ~ChannelSession();

    ChannelSession *clone();

    virtual Type type() const override {
        return CHAN;
    }

    virtual void next_in(m3::GateIStream &is) override {
        SLOG(VTERM, "[" << id << "] vterm::next_in()");

        if(writing) {
            reply_error(is, Errors::NO_PERM);
            return;
        }

        pos += len - pos;

        Errors::last = Errors::NONE;
        if(pos == len) {
            char buf[BUF_SIZE];
            len = static_cast<size_t>(Machine::read(buf, sizeof(buf)));
            mem.write(buf, len, 0);
            pos = 0;
        }

        if(!active) {
            Syscalls::activate(ep, mem.sel(), 0);
            active = true;
        }

        if(Errors::last != Errors::NONE)
            reply_error(is, Errors::last);
        else
            reply_vmsg(is, Errors::NONE, pos, len - pos);
    }

    virtual void next_out(m3::GateIStream &is) override {
        SLOG(VTERM, "[" << id << "] vterm::next_out()");

        if(!writing) {
            reply_error(is, Errors::NO_PERM);
            return;
        }

        if(len > 0) {
            char buf[BUF_SIZE];
            mem.read(buf, len, 0);
            Machine::write(buf, len);
            len = 0;
        }

        if(!active) {
            Syscalls::activate(ep, mem.sel(), 0);
            active = true;
        }

        if(Errors::last != Errors::NONE)
            reply_error(is, Errors::last);
        else {
            pos = 0;
            len = BUF_SIZE;
            reply_vmsg(is, Errors::NONE, static_cast<size_t>(0), BUF_SIZE);
        }
    }

    virtual void commit(m3::GateIStream &is, size_t nbytes) override {
        SLOG(VTERM, "[" << id << "] vterm::commit(nbytes=" << nbytes << ")");

        if(nbytes > len - pos) {
            reply_error(is, Errors::INV_ARGS);
            return;
        }

        if(writing) {
            if(len > 0) {
                char buf[BUF_SIZE];
                mem.read(buf, nbytes, 0);
                Machine::write(buf, nbytes);
                len = 0;
            }
            reply_vmsg(is, Errors::NONE);
        }
        else {
            pos += nbytes;
            reply_vmsg(is, Errors::NONE);
        }
    }

    uint id;
    bool active;
    bool writing;
    capsel_t ep;
    SendGate sgate;
    MemGate mem;
    size_t pos;
    size_t len;
};

class VTermHandler : public base_class {
public:
    explicit VTermHandler(WorkLoop *wl)
        : base_class(),
          _slots(),
          _mem(MemGate::create_global(MAX_CLIENTS * BUF_SIZE, MemGate::RW)),
          _rgate(RecvGate::create(nextlog2<MAX_CLIENTS * MSG_SIZE>::val, nextlog2<MSG_SIZE>::val)) {
        add_operation(GenericFile::SEEK, &VTermHandler::invalid_op);
        add_operation(GenericFile::STAT, &VTermHandler::invalid_op);
        add_operation(GenericFile::NEXT_IN, &VTermHandler::next_in);
        add_operation(GenericFile::NEXT_OUT, &VTermHandler::next_out);
        add_operation(GenericFile::COMMIT, &VTermHandler::commit);
        add_operation(GenericFile::CLOSE, &VTermHandler::close_chan);

        using std::placeholders::_1;
        _rgate.start(wl, std::bind(&VTermHandler::handle_message, this, _1));
    }

    virtual Errors::Code open(VTermSession **sess, capsel_t srv_sel, word_t) override {
        *sess = new MetaSession(srv_sel);
        return Errors::NONE;
    }

    virtual Errors::Code obtain(VTermSession *sess, KIF::Service::ExchangeData &data) override {
        if(data.caps != 1 && data.caps != 2)
            return Errors::INV_ARGS;

        ChannelSession *nsess;
        if(sess->type() == VTermSession::META) {
            if(data.args.count != 1)
                return Errors::INV_ARGS;
            nsess = static_cast<MetaSession*>(sess)->create_chan(data.args.vals[0] == 1);
        }
        else {
            if(data.args.count != 0)
                return Errors::INV_ARGS;
            nsess = static_cast<ChannelSession*>(sess)->clone();
        }
        if(nsess == nullptr)
            return Errors::NO_SPACE;

        data.caps = KIF::CapRngDesc(KIF::CapRngDesc::OBJ, nsess->sel(), 2).value();
        return Errors::NONE;
    }

    virtual Errors::Code delegate(VTermSession *sess, KIF::Service::ExchangeData &data) override {
        if(data.caps != 1 || data.args.count != 0 || sess->type() != VTermSession::CHAN)
            return Errors::INV_ARGS;

        ChannelSession *chan = static_cast<ChannelSession*>(sess);
        chan->ep = VPE::self().alloc_sel();
        data.caps = KIF::CapRngDesc(KIF::CapRngDesc::OBJ, chan->ep, 1).value();
        return Errors::NONE;
    }

    virtual Errors::Code close(VTermSession *sess) override {
        delete sess;
        return Errors::NONE;
    }

    virtual void shutdown() override {
        _rgate.stop();
    }

    void invalid_op(GateIStream &is) {
        reply_vmsg(is, m3::Errors::NOT_SUP);
    }

    void next_in(m3::GateIStream &is) {
        VTermSession *sess = is.label<VTermSession*>();
        sess->next_in(is);
    }

    void next_out(m3::GateIStream &is) {
        VTermSession *sess = is.label<VTermSession*>();
        sess->next_out(is);
    }

    void commit(m3::GateIStream &is) {
        VTermSession *sess = is.label<VTermSession*>();
        size_t nbytes;
        is >> nbytes;

        sess->commit(is, nbytes);
    }

    void close_chan(m3::GateIStream &is) {
        VTermSession *sess = is.label<VTermSession*>();
        close(sess);
        reply_error(is, Errors::NONE);
    }

    ChannelSession *new_chan(bool write) {
        // find and allocate free slot
        uint id = _slots.first_clear();
        if(id == MAX_CLIENTS)
            return nullptr;
        _slots.set(id, true);

        capsel_t caps = VPE::self().alloc_sels(2);
        return new ChannelSession(id, _rgate, _mem, srv->sel(), caps, write);
    }

    BitField<MAX_CLIENTS> _slots;
    MemGate _mem;
    RecvGate _rgate;
};

inline ChannelSession *MetaSession::create_chan(bool write) {
    return srv->handler().new_chan(write);
}

inline ChannelSession::~ChannelSession() {
    srv->handler()._slots.set(id, false);
}
inline ChannelSession *ChannelSession::clone() {
    return srv->handler().new_chan(writing);
}

int main() {
    WorkLoop wl;

    srv = new Server<VTermHandler>("vterm", &wl, new VTermHandler(&wl));
    wl.run();
    delete srv;

    return 0;
}
