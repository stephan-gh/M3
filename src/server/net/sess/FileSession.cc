/*
 * Copyright (C) 2018, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
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

#include "FileSession.h"
#include "SocketSession.h"

#include "lwip/pbuf.h"

using namespace m3;

template<typename... Args>
static void reply_vmsg_late(RecvGate &rgate, const DTU::Message *msg, const Args &... args) {
    auto reply = create_vmsg(args...);
    size_t idx = DTU::get().get_msgoff(rgate.ep(), msg);
    rgate.reply(reply.bytes(), reply.total(), idx);
}

FileSession::FileSession(capsel_t srv_sel, LwipSocket* socket, int mode, size_t rmemsize, size_t smemsize)
    : NMSession(srv_sel, VPE::self().alloc_sels(2)),
      _work_item(*this),
      _sgate(new SendGate(SendGate::create(&socket->session()->rgate(), reinterpret_cast<label_t>(this),
                                           MSG_SIZE, nullptr, sel() + 1))),
      _socket(socket),
      _memory(nullptr),
      _mode(mode),
      _rbuf(rmemsize),
      _sbuf(smemsize),
      _lastamount(0),
      _sending(false),
      _pending(nullptr),
      _pending_gate(nullptr),
      _client_memep(ObjCap::INVALID),
      _client_memgate(nullptr) {
    m3::env()->workloop()->add(&_work_item, false);
}

FileSession::~FileSession()  {
    m3::env()->workloop()->remove(&_work_item);

    if(_pending && _pending_gate) {
        // send eof
        reply_vmsg_late(*_pending_gate, _pending, Errors::NONE, (size_t)0, (size_t)0);
    }

    delete _sgate;
    delete _client_memgate;
    delete _memory;
}

m3::Errors::Code FileSession::delegate(m3::KIF::Service::ExchangeData& data) {
    // Client delegates shared memory to us
    if(data.caps == 1 && data.args.count == 1) {
        capsel_t sel = VPE::self().alloc_sel();
        _memory = new MemGate(MemGate::bind(sel));
        data.caps = KIF::CapRngDesc(KIF::CapRngDesc::OBJ, sel, data.caps).value();
        return Errors::NONE;
    // Client delegates a memory endpoint to us for configuration
    } else if(data.caps == 1 && data.args.count == 0) {
        capsel_t sel = VPE::self().alloc_sel();
        _client_memep = sel;
        data.caps = KIF::CapRngDesc(KIF::CapRngDesc::OBJ, sel, data.caps).value();
        return Errors::NONE;
    } else
        return Errors::INV_ARGS;
}

bool FileSession::is_recv() {
    return _mode & FILE_R;
}

bool FileSession::is_send() {
    return _mode & FILE_W;
}

// Support clone?

Errors::Code FileSession::activate() {
    if(_client_memep != ObjCap::INVALID) {
        if(_memory == nullptr)
            return Errors::INV_ARGS;

        if(_client_memgate == nullptr) {
            _client_memgate = new MemGate(_memory->derive(0, _rbuf.size() + _sbuf.size(), MemGate::RW));
        }

        if(Syscalls::get().activate(_client_memep, _client_memgate->sel(), 0) != Errors::NONE)
            return Errors::last;
        _client_memep = ObjCap::INVALID;
    }
    return Errors::NONE;
}

Errors::Code FileSession::prepare() {
    if(_pending != 0) {
        LOG_SESSION(this, "already has a pending request");
        return Errors::INV_STATE;
    }

    return activate();
}

void FileSession::next_in(m3::GateIStream& is) {
    if(!is_recv()) {
        reply_error(is, Errors::NOT_SUP);
        return;
    }

    Errors::Code res = prepare();
    if(res != Errors::NONE) {
        reply_error(is, res);
        return;
    }

    if(/* TODO: socket is closed */ false) {
        LOG_SESSION(this, "recv: EOF");
        reply_vmsg(is, Errors::NONE, (size_t)0, (size_t)0);
        return;
    }

    // implicitly commit the previous in request
    if(!_sending && _lastamount != 0) {
        LOG_SESSION(this, "recv: implicit commit of previous recv"
                          << " (" << _lastamount << ")");
        Errors::Code res = commit(_lastamount);
        if(res != Errors::NONE) {
            reply_error(is, res);
            return;
        }
    }

    _sending = false;

    size_t amount = get_recv_size();
    ssize_t pos = _rbuf.get_read_pos(&amount);
    if(pos == -1) {
        LOG_SESSION(this, "recv: waiting for data");
        mark_pending(is);
    } else {
        _lastamount = amount;
        LOG_SESSION(this, "recv: " << amount << " @" << pos);
        reply_vmsg(is, Errors::NONE, pos, amount);
    }
}

void FileSession::next_out(m3::GateIStream& is) {
    if(!is_send()) {
        reply_error(is, Errors::NOT_SUP);
        return;
    }

    Errors::Code res = prepare();
    if(res != Errors::NONE) {
        reply_error(is, res);
        return;
    }

    if(/* TODO: socket is closed */ false) {
        LOG_SESSION(this, "send: EOF");
        reply_vmsg(is, Errors::NONE, (size_t)0, (size_t)0);
        return;
    }

    // implicitly commit the previous in/out request
    if(_lastamount != 0) {
        LOG_SESSION(this, "send: implicit commit of previous "
                          << (_sending ? "send" : "recv")
                          << " (" << _lastamount << ")");
        Errors::Code res = commit(_lastamount);
        if(res != Errors::NONE) {
            reply_error(is, res);
            return;
        }
    }

    _sending = true;

    size_t amount = get_send_size();
    ssize_t pos = _sbuf.get_write_pos(amount);
    // TODO: Maybe fallback to a smaller chunk?
    if(pos == -1) {
        LOG_SESSION(this, "send: waiting for free memory");
        mark_pending(is);
    } else {
        _lastamount = amount;
        LOG_SESSION(this, "send: " << amount << " @" << pos);
        reply_vmsg(is, Errors::NONE, _rbuf.size() + static_cast<size_t>(pos), amount);
    }
}

void FileSession::commit(m3::GateIStream& is) {
    Errors::Code res = prepare();
    if(res != Errors::NONE) {
        reply_error(is, res);
        return;
    }

    size_t amount;
    is >> amount;
    if(amount == 0) {
        reply_error(is, Errors::INV_ARGS);
        return;
    }

    res = commit(amount);
    if(_sending) {
        reply_vmsg(is, res, _sbuf.size());
    } else {
        reply_vmsg(is, res, _rbuf.size());
    }
}

void FileSession::close(m3::GateIStream &is) {
    reply_error(is, Errors::NONE);
}

Errors::Code FileSession::commit(size_t amount) {
    if(amount != 0 && amount > _lastamount)
        return Errors::INV_ARGS;

    if(_sending) {
        // Advance write pointer
        _sbuf.push(_lastamount, amount);
    } else {
        // Advance read pointer
        _rbuf.pull(amount != 0 ? amount : _lastamount);
    }

    _lastamount = 0;

    return Errors::NONE;
}

size_t FileSession::get_recv_size() const {
    return _rbuf.size() / 4;
}

size_t FileSession::get_send_size() const {
    return _sbuf.size() / 4;
}

m3::Errors::Code FileSession::handle_recv(struct pbuf* p) {
    if(!_memory)
        return Errors::OUT_OF_MEM;

    size_t amount = p->tot_len;
    ssize_t pos = _rbuf.get_write_pos(amount);
    if(pos != -1) {
        // Verify that p is a continuous chunk of memory!
        if(!p->next) {
            _memory->write(p->payload, amount, static_cast<goff_t>(pos));
            _rbuf.push(amount, amount);
            return Errors::NONE;
        } else {
            LOG_SESSION(this, "handle_recv: pbuf has to be a continuous chunk of memory");
            return Errors::INV_ARGS;
        }
    } else
        return Errors::OUT_OF_MEM;
}

void FileSession::mark_pending(m3::GateIStream& is) {
    assert(_pending == 0);
    _pending = &is.message();
    _pending_gate = &is.rgate();
    is.claim();
}

FileSession::WorkItem::WorkItem(FileSession& session)
    : _session(session) {
}

void FileSession::WorkItem::work() {
    _session.handle_send_buffer();
    _session.handle_pending_recv();
    _session.handle_pending_send();
}

void FileSession::handle_send_buffer() {
    // Process multiple chunks per invocation?
    size_t amount = get_send_size();
    ssize_t pos = _sbuf.get_read_pos(&amount);
    if(pos != -1) {
        LOG_SESSION(this, "handle_send_buffer: amount=" << amount << ", pos=" << pos);
        ssize_t res = _socket->send_data(*_memory, _rbuf.size() + static_cast<size_t>(pos), amount);
        if(res > 0)
            _sbuf.pull(static_cast<size_t>(res));
    }
}

void FileSession::handle_pending_recv() {
    if(!_pending || _sending)
        return;

    size_t amount = get_recv_size();
    ssize_t pos = _rbuf.get_read_pos(&amount);
    if(pos != -1) {
        _lastamount = amount;
        LOG_SESSION(this, "late-recv: " << amount << " @" << pos);
        reply_vmsg_late(*_pending_gate, _pending, Errors::NONE, pos, amount);
        _pending = nullptr;
        _pending_gate = nullptr;
    }
}

void FileSession::handle_pending_send() {
  if (!_pending || !_sending) return;

  size_t amount = get_send_size();
  ssize_t pos = _sbuf.get_write_pos(amount);
  // TODO: Maybe fallback to a smaller chunk?
  if (pos != -1) {
    _lastamount = amount;
    LOG_SESSION(this, "late-send: " << amount << " @" << pos);
    reply_vmsg_late(*_pending_gate, _pending, Errors::NONE, pos, amount);
    _pending = nullptr;
    _pending_gate = nullptr;
  }
}
