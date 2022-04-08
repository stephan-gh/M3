/*
 * Copyright (C) 2021 Nils Asmussen, Barkhausen Institut
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

#include <m3/net/Net.h>
#include <m3/net/TcpSocket.h>
#include <m3/net/UdpSocket.h>
#include <m3/session/NetworkManager.h>

class OpHandler {
public:
    virtual ~OpHandler() {}

    virtual void send(const void *data, size_t len) = 0;
};

class TCPOpHandler : public OpHandler {
public:
    explicit TCPOpHandler(m3::NetworkManager &nm, m3::IpAddr ip, m3::port_t port);

    virtual void send(const void *data, size_t len) override;

private:
    m3::FileRef<m3::TcpSocket> _socket;
};

class UDPOpHandler : public OpHandler {
public:
    explicit UDPOpHandler(m3::NetworkManager &nm, m3::IpAddr ip, m3::port_t port);

    virtual void send(const void *data, size_t len) override;

private:
    m3::Endpoint _ep;
    m3::FileRef<m3::UdpSocket> _socket;
};
