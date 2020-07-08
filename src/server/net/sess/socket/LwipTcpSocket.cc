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

#include "lwip/ip_addr.h"
#include "lwip/pbuf.h"
#include "lwip/tcp.h"

#include "../FileSession.h"
#include "../SocketSession.h"

#include "LwipTcpSocket.h"

using namespace m3;

LwipTcpSocket::WorkItem::WorkItem(LwipTcpSocket& socket)
    : _socket(socket) {
}

void LwipTcpSocket::WorkItem::work() {
    _socket.flush_data();
}

LwipTcpSocket::LwipTcpSocket(WorkLoop *wl, SocketSession *session)
    : LwipSocket(session),
     _wl(wl),
     _pcb(nullptr),
     _work_item(*this) {
    // TODO: Evaluate if the work item is really needed
    wl->add(&_work_item, false);
}

LwipTcpSocket::~LwipTcpSocket() {
    if(_pcb != nullptr) {
        if(close() != Errors::NONE && _pcb != nullptr) {
            LOG_SOCKET(this, "Abort connection, because gracefully closing the socket failed.");
            tcp_abort(_pcb);
            _pcb = nullptr;
        }
    }
}

void LwipTcpSocket::eof() {
    if(rfile())
        rfile()->handle_eof();
    if(sfile())
        sfile()->handle_eof();
}

m3::Errors::Code LwipTcpSocket::create(uint8_t protocol) {
    if(protocol != 0 && protocol != IP_PROTO_TCP) {
        LOG_SOCKET(this, "create failed: invalid protocol");
        return Errors::INV_ARGS;
    }
    _pcb = tcp_new();
    if(!_pcb) {
        LOG_SOCKET(this, "create failed: allocation of pcb failed");
        return Errors::NO_SPACE;
    }

    // set argument for callback functions
    tcp_arg(_pcb, this);
    tcp_err(_pcb, LwipTcpSocket::tcp_err_cb);

    return Errors::NONE;
}

m3::Errors::Code LwipTcpSocket::bind(ip4_addr addr, uint16_t port) {
    err_t err = tcp_bind(_pcb, &addr, port);
    if(err != ERR_OK)
        LOG_SOCKET(this, "bind failed: " << errToStr(err));

    return(mapError(err));
}

m3::Errors::Code LwipTcpSocket::listen() {
    err_t err = ERR_OK;
    struct tcp_pcb *lpcb = tcp_listen_with_backlog_and_err(
        _pcb, MAX_SOCKET_BACKLOG, &err);

    if(lpcb)
        _pcb = lpcb;

    if(err == ERR_OK) {
        tcp_accept(_pcb, LwipTcpSocket::tcp_accept_cb);
    } else
        LOG_SOCKET(this, "listen failed: " << errToStr(err));

    return mapError(err);
}

m3::Errors::Code LwipTcpSocket::connect(ip4_addr addr, uint16_t port) {
    // Set recv and sent callback
    tcp_recv(_pcb, LwipTcpSocket::tcp_recv_cb);
    tcp_sent(_pcb, LwipTcpSocket::tcp_sent_cb);

    err_t err = tcp_connect(_pcb, &addr, port, LwipTcpSocket::tcp_connected_cb);
     if(err != ERR_OK)
         LOG_SOCKET(this, "connect failed: " << errToStr(err));
     return mapError(err);
}

m3::Errors::Code LwipTcpSocket::close() {
    err_t err = tcp_close(_pcb);
    if(err == ERR_OK) {
        // to be safe: don't call the callback with this anymore
        tcp_arg(_pcb, nullptr);
        _pcb = nullptr;
    }
    else
        LOG_SOCKET(this, "close failed: " << errToStr(err));
    return mapError(err);
}

ssize_t LwipTcpSocket::send_data_internal(const void * data, size_t size) {
    u16_t len = static_cast<u16_t>(size);
    err_t err = tcp_write(_pcb, data, len, TCP_WRITE_FLAG_COPY);
    LOG_SOCKET(this, "tcp_write it " << err);
    // tcp_write does does not trigger sending of TCP segements.
    tcp_output(_pcb);
    if(err != ERR_OK) {
        LOG_SOCKET(this, "send_data failed: " << errToStr(err));
        return -1;
    }
    return len;
}

ssize_t LwipTcpSocket::send_data(const void * data, size_t size) {
    // Try to empty queue
    flush_data();

    // Queue has to be empty, we do not want to send data out of order.
    if(_send_queue.has_data())
        return -1;

    return send_data_internal(data, size);
}

void LwipTcpSocket::enqueue_data(m3::DataQueue::Item *item) {
    LOG_SOCKET(this, "Enqueue " << item->get_size() - item->get_pos() << " bytes into send queue.");
    _send_queue.append(item);
}

// TODO: Call when tcp_write() can take new input (Timeout?, lwIP callback?, Workloop item?)
void LwipTcpSocket::flush_data() {
    while(_send_queue.has_data()) {
        const uchar * data = nullptr;
        size_t size = 0;
        if(_send_queue.get_next_data(data, size)) {
            LOG_SOCKET(this, "tcp_sent_cb: processing send queue (size=" << size << ")");
            // TODO: Prevent deadlock, if size is too large when tcp_sent_cb is called.
            ssize_t sent_size = send_data_internal(data, size);
            if(sent_size <= 0)
                break;
            _send_queue.ack_data(static_cast<size_t>(sent_size));
        } else
            break;
    }
}

void LwipTcpSocket::tcp_err_cb(void* arg, err_t err) {
    LwipTcpSocket * socket = static_cast<LwipTcpSocket *>(arg);
    if(socket == nullptr)
        return;

    LOG_SOCKET(socket, "tcp_err_cb: " << errToStr(err));

    // ERR_ABRT: aborted through tcp_abort or by a TCP timer
    // ERR_RST: the connection was reset by the remote host
    // TODO: Handle failure
    socket->eof();
    socket->_channel->socket_closed(socket->_sd, mapError(err));
}

err_t LwipTcpSocket::tcp_accept_cb(void *arg, struct tcp_pcb *newpcb, err_t err) {
    LwipTcpSocket * socket = static_cast<LwipTcpSocket *>(arg);
    if(socket == nullptr)
        return ERR_ABRT;

    LOG_SOCKET(socket, "tcp_accept_cb");

    if(err != ERR_OK) {
        LOG_SOCKET(socket, "tcp_accept_cb failed: " << errToStr(err));
        return ERR_OK;
    }

    LwipTcpSocket * new_socket = new LwipTcpSocket(socket->_wl, socket->_session);
    new_socket->_pcb = newpcb;
    new_socket->_channel = socket->_channel;
    tcp_arg(newpcb, new_socket);
    tcp_err(newpcb, LwipTcpSocket::tcp_err_cb);
    tcp_recv(newpcb, LwipTcpSocket::tcp_recv_cb);
    tcp_sent(newpcb, LwipTcpSocket::tcp_sent_cb);

    int new_sd = socket->session()->request_sd(new_socket);
    if(new_sd == -1) {
        delete new_socket;
        LOG_SOCKET(socket, "tcp_accept_cb failed: maximum number of sockets reached");
        // Abort accept
        tcp_abort(newpcb);
        return ERR_ABRT;
    } else {
        // TODO: Handle failure
        new_socket->_channel->socket_accept(socket->_sd, new_socket->_sd, IpAddr(lwip_ntohl(newpcb->remote_ip.addr)), newpcb->remote_port);
        return ERR_OK;
    }
}

err_t LwipTcpSocket::tcp_connected_cb(void* arg, struct tcp_pcb* tpcb, err_t err) {
    LWIP_UNUSED_ARG(tpcb);
    LWIP_UNUSED_ARG(err); // error code is always ERR_OK

    LwipTcpSocket * socket = static_cast<LwipTcpSocket *>(arg);
    if(socket == nullptr)
        return ERR_ABRT;

    LOG_SOCKET(socket, "tcp_connected_cb: " << errToStr(err));

    // TODO: Handle failure
    socket->_channel->socket_connected(socket->_sd);

    return ERR_OK;
}

err_t LwipTcpSocket::tcp_recv_cb(void* arg, struct tcp_pcb* tpcb, struct pbuf* p, err_t err) {
    LWIP_UNUSED_ARG(tpcb);
    LWIP_UNUSED_ARG(err); // error code is always ERR_OK

    LwipTcpSocket * socket = static_cast<LwipTcpSocket *>(arg);
    if(socket == nullptr)
        return ERR_ABRT;

    // The connection has been closed!
    if(p == NULL) {
        LOG_SOCKET(socket, "tcp_recv_cb: connection has been closed");
        // TODO: Handle failure
        socket->eof();
        socket->_channel->socket_closed(socket->_sd, Errors::CONN_CLOSED);
        return ERR_OK;
    }

    // TODO: Add option to mark socket as file exclusive, so that inband data transfer is not used.
    // Otherwise data that is received before a file session is opened could go missing.
    Errors::Code res;
    if(socket->_rfile) {
        LOG_SOCKET(socket, "tcp_recv_cb: using recv file session");
        res = socket->_rfile->handle_recv(p);
    } else {
        LOG_SOCKET(socket, "tcp_recv_cb: using inband data transfer");
        // TODO: Split into multiple inband data transfers if necessary.
        bool success = socket->_channel->inband_data_transfer(socket->_sd, p->tot_len, [&](uchar * buf) {
            pbuf_copy_partial(p, buf, p->tot_len, 0);
        });
        res = success ? Errors::NONE : Errors::NO_CREDITS;
    }

    if(res == Errors::NONE) {
        LOG_SOCKET(socket, "tcp_recv_cb: received " << p->tot_len << " bytes");

        // Inform lwIP that we have processed the data.
        tcp_recved(socket->_pcb, p->tot_len);
        pbuf_free(p);
        return ERR_OK;
    } else {
        LOG_SOCKET(socket, "tcp_recv_cb: can not pass received data to client: " << Errors::to_string(res));
        // don't deallocate p: it is presented to us later again from tcp_fasttmr!
        return ERR_MEM;
    }
}

err_t LwipTcpSocket::tcp_sent_cb(void *arg, struct tcp_pcb *tpcb, u16_t len) {
    LWIP_UNUSED_ARG(tpcb);

    LwipTcpSocket * socket = static_cast<LwipTcpSocket *>(arg);
    if(socket == nullptr)
        return ERR_ABRT;

    LOG_SOCKET(socket, "tcp_sent_cb: " << len);

    socket->flush_data();

    return ERR_OK;
}
