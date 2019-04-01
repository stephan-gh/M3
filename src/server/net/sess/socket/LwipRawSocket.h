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

#include "LwipSocket.h"

class LwipRawSocket : public LwipSocket {
public:
    explicit LwipRawSocket(SocketSession *session)
        : LwipSocket(session),
         _pcb(nullptr) {
    }

    virtual ~LwipRawSocket() {
        if(_pcb != nullptr)
            close();
    }

    virtual m3::Socket::SocketType type() const override {
        return m3::Socket::SOCK_RAW;
    }

    virtual m3::Errors::Code create(uint8_t protocol) override;
    virtual m3::Errors::Code bind(ip4_addr addr, uint16_t port) override;
    virtual m3::Errors::Code listen() override;
    virtual m3::Errors::Code connect(ip4_addr ip_addr, uint16_t port) override;
    virtual m3::Errors::Code close() override;

    virtual ssize_t send_data(const void * data, size_t size) override;
private:
    struct raw_pcb * _pcb;
};
