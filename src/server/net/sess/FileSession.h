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

#pragma once

#include <base/Common.h>
#include <base/util/VarRingBuf.h>

#include <m3/session/NetworkManager.h>
#include <m3/session/ServerSession.h>

#include "Session.h"

class LwipSocket;

class FileSession : public NMSession {
    class WorkItem : public m3::WorkItem {
    public:
        WorkItem(FileSession & session);
        virtual void work() override;
    private:
        FileSession & _session;
    };
public:
    explicit FileSession(capsel_t srv_sel, LwipSocket * socket, int mode,
                         size_t rmemsize, size_t smemsize);

    ~FileSession();

    virtual Type type() const override {
        return FILE;
    }

    virtual m3::Errors::Code delegate(m3::KIF::Service::ExchangeData &data) override;

    /**
     * @return Selectors for session and client send gate capabilities.
     */
    m3::KIF::CapRngDesc caps() const {
        return m3::KIF::CapRngDesc(m3::KIF::CapRngDesc::OBJ, sel(), 2);
    }

    bool is_recv();
    bool is_send();

    virtual void next_in(m3::GateIStream &is) override;
    virtual void next_out(m3::GateIStream &is) override;
    virtual void commit(m3::GateIStream &is) override;
    virtual void close(m3::GateIStream &is) override;

    m3::Errors::Code handle_recv(struct pbuf* p);

private:
    m3::Errors::Code activate();
    m3::Errors::Code prepare();
    m3::Errors::Code commit(size_t amount);

    size_t get_recv_size() const;
    size_t get_send_size() const;
    void mark_pending(m3::GateIStream &is);

    void handle_send_buffer();
    void handle_pending_recv();
    void handle_pending_send();

private:
    WorkItem _work_item;

    m3::SendGate _sgate;
    LwipSocket * _socket;
    // Shared memory provided by client
    m3::MemGate *_memory;
    // File mode
    int _mode;
    // Manages data in RX direction (memory[0] to memory[_rbuf.size() - 1])
    VarRingBuf _rbuf;
    // Manages data in TX direction (memory[_rbuf.size()] to memory[_rbuf.size() + _sbuf.size() - 1])
    VarRingBuf _sbuf;
    // Amount of memory returned by the last next_in/next_out
    size_t _lastamount;
    // Client is currently sending data (writing to _sbuf)
    bool _sending;
    // Pending recv/send request
    m3::DTU::Message const * _pending;
    m3::RecvGate * _pending_gate;
    // Memory endpoint provided by client to us for configuration
    capsel_t _client_memep;
    // Memory gate activated for client
    m3::MemGate * _client_memgate;
};
