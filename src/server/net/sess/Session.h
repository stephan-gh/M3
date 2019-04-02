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
#include <base/col/SList.h>
#include <base/log/Services.h>

#include <m3/com/GateStream.h>
#include <m3/session/ServerSession.h>

#define LOG_SESSION(sess, msg)  SLOG(NET, fmt((word_t)sess, "#x") << ": " << msg)

class NMSession : public m3::ServerSession, public m3::SListItem {
public:
    static constexpr size_t MSG_SIZE = 128;

    enum Type {
        SOCKET,
        FILE,
    };

    explicit NMSession(capsel_t srv_sel, capsel_t sel = m3::ObjCap::INVALID)
        : m3::ServerSession(srv_sel, sel), m3::SListItem() {
    }
    virtual ~NMSession() {
    }

    virtual Type type() const = 0;

    virtual m3::Errors::Code obtain(capsel_t, m3::KIF::Service::ExchangeData &) {
        return m3::Errors::NOT_SUP;
    }
    virtual m3::Errors::Code delegate(m3::KIF::Service::ExchangeData &) {
        return m3::Errors::NOT_SUP;
    }

    virtual void create(m3::GateIStream &is) {
        m3::reply_error(is, m3::Errors::NOT_SUP);
    }
    virtual void bind(m3::GateIStream &is) {
        m3::reply_error(is, m3::Errors::NOT_SUP);
    }
    virtual void listen(m3::GateIStream &is) {
        m3::reply_error(is, m3::Errors::NOT_SUP);
    }
    virtual void connect(m3::GateIStream &is) {
        m3::reply_error(is, m3::Errors::NOT_SUP);
    }
    virtual void close(m3::GateIStream &is) {
        m3::reply_error(is, m3::Errors::NOT_SUP);
    }
    virtual void next_in(m3::GateIStream &is) {
        m3::reply_error(is, m3::Errors::NOT_SUP);
    }
    virtual void next_out(m3::GateIStream &is) {
        m3::reply_error(is, m3::Errors::NOT_SUP);
    }
    virtual void commit(m3::GateIStream &is) {
        m3::reply_error(is, m3::Errors::NOT_SUP);
    }
    virtual void seek(m3::GateIStream &is) {
        m3::reply_error(is, m3::Errors::NOT_SUP);
    }
    virtual void stat(m3::GateIStream &is) {
        m3::reply_error(is, m3::Errors::NOT_SUP);
    }
};
