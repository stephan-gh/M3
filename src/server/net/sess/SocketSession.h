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

#include <m3/session/NetworkManager.h>
#include <m3/session/ServerSession.h>

#include "Session.h"
#include "socket/LwipSocket.h"

class NetEventChannelWorkItem : public m3::WorkItem {
public:
    explicit NetEventChannelWorkItem(m3::NetEventChannel & channel, SocketSession & session)
        : _channel(channel), _session(session) {
    }

    virtual void work() override;

protected:
    m3::NetEventChannel & _channel;
    SocketSession & _session;
};

class SocketSession : public NMSession {
public:
    static constexpr size_t MAX_SEND_RECEIVE_BATCH_SIZE = 5;
    static constexpr size_t MAX_SOCKETS                 = 16;

    explicit SocketSession(m3::WorkLoop *wl, capsel_t srv_sel, m3::RecvGate &rgate);
    ~SocketSession();

    virtual Type type() const override {
        return SOCKET;
    }

    m3::RecvGate & rgate() {
        return _rgate;
    }

    m3::Errors::Code obtain(capsel_t srv_sel, m3::KIF::Service::ExchangeData &data) override;
    m3::Errors::Code get_sgate(m3::KIF::Service::ExchangeData &data);
    m3::Errors::Code establish_channel(m3::KIF::Service::ExchangeData &data);
    m3::Errors::Code open_file(capsel_t srv_sel, m3::KIF::Service::ExchangeData &data);

    virtual void create(m3::GateIStream &is) override;
    virtual void bind(m3::GateIStream &is) override;
    virtual void listen(m3::GateIStream &is) override;
    virtual void connect(m3::GateIStream &is) override;
    virtual void close(m3::GateIStream &is) override;

    LwipSocket *get_socket(int sd);
    int request_sd(LwipSocket *socket);
    void release_sd(int sd);

private:
    m3::WorkLoop *_wl;
    m3::SendGate *_sgate;
    m3::RecvGate &_rgate;
    capsel_t _channel_caps;
    m3::NetEventChannel * _channel;
    NetEventChannelWorkItem * _channelWorkItem;
    LwipSocket *sockets[MAX_SOCKETS];
};
