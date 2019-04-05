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

class LwipTcpSocket : public LwipSocket {
    class WorkItem : public m3::WorkItem {
    public:
        WorkItem(LwipTcpSocket & session);
        virtual void work() override;
    private:
        LwipTcpSocket & _socket;
    };
public:
    static constexpr size_t MAX_SOCKET_BACKLOG = 10;

    explicit LwipTcpSocket(m3::WorkLoop *wl, SocketSession *session);
    virtual ~LwipTcpSocket();

    virtual m3::Socket::SocketType type() const override {
        return m3::Socket::SOCK_STREAM;
    }

    virtual m3::Errors::Code create(uint8_t protocol) override;
    virtual m3::Errors::Code bind(ip4_addr addr, uint16_t port) override;
    virtual m3::Errors::Code listen() override;
    virtual m3::Errors::Code connect(ip4_addr addr, uint16_t port) override;
    virtual m3::Errors::Code close() override;

    virtual ssize_t send_data(const void * data, size_t size) override;
    virtual void enqueue_data(m3::DataQueue::Item *item) override;

    void flush_data();

private:
    ssize_t send_data_internal(const void * data, size_t size);

private:
    static void tcp_err_cb(void *arg, err_t err);
    static err_t tcp_accept_cb(void *arg, struct tcp_pcb *newpcb, err_t err);
    static err_t tcp_connected_cb(void *arg, struct tcp_pcb *tpcb, err_t err);
    static err_t tcp_recv_cb(void *arg, struct tcp_pcb *tpcb, struct pbuf *p, err_t err);
    static err_t tcp_sent_cb(void *arg, struct tcp_pcb *tpcb, u16_t len);

private:
    m3::WorkLoop *_wl;
    struct tcp_pcb * _pcb;
    m3::DataQueue _send_queue;
    WorkItem _work_item;
};
