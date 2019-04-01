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
#include <base/log/Services.h>

#include <m3/session/NetworkManager.h>

#include "lwip/err.h"
#include "lwip/ip4_addr.h"

class SocketSession;
class FileSession;

#define LOG_SOCKET(socket, msg)  SLOG(NET, fmt((word_t)socket->session(), "#x") << "(" << socket->sd() << "): " << msg)

class LwipSocket {
    friend class SocketSession;

public:
    explicit LwipSocket(SocketSession *session)
        : _sd(-1),
          _session(session),
          _channel(nullptr),
          _rfile(nullptr),
          _sfile(nullptr){
    }

    virtual ~LwipSocket();

    virtual m3::Socket::SocketType type() const = 0;

    int sd() {
        return _sd;
    }

    SocketSession * session() {
        return _session;
    }

    m3::NetEventChannel * channel() {
        return _channel;
    }

    void channel(m3::NetEventChannel * channel) {
        _channel = channel;
    }

    FileSession * rfile() {
        return _rfile;
    }

    FileSession * sfile() {
        return _sfile;
    }

    virtual m3::Errors::Code create(uint8_t protocol) = 0;
    virtual m3::Errors::Code bind(ip4_addr addr, uint16_t port) = 0;
    virtual m3::Errors::Code listen() = 0;
    virtual m3::Errors::Code connect(ip4_addr ip_addr, uint16_t port) = 0;
    virtual m3::Errors::Code close() = 0;

    /**
     *
     * @param data
     * @param size
     * @return result <= 0: Processing failed.
     *         0 < result < size: Only a part of the data has been processed.
     *         result == size: All data has been processed.
     * If a socket implementation potentially returns result != size,
     * the caller can put the remaining data into the buffers send queue.
     * Therefore such a socket implementation has to process the send queue.
     */
    virtual ssize_t send_data(const void * data, size_t size) = 0;
    virtual ssize_t send_data(m3::MemGate & mem, goff_t offset, size_t size);

    virtual void enqueue_data(m3::DataQueue::Item *item);

protected:
    static err_t errToStr(err_t err);
    static m3::Errors::Code mapError(err_t err);

    int _sd;
    SocketSession * _session;
    m3::NetEventChannel * _channel;
    FileSession * _rfile;
    FileSession * _sfile;
private:
    void set_sd(int sd) {
        _sd = sd;
    }
};
