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

#include <base/col/List.h>
#include <base/col/Treap.h>
#include <m3/net/DataQueue.h>

#include <m3/net/Net.h>
#include <m3/net/NetEventChannel.h>

namespace m3 {

class NetworkManager;
class DataQueue;

class Socket : public m3::TreapNode<Socket, int>, public SListItem {
    friend NetworkManager;

public:
    static const int EVENT_FETCH_BATCH_SIZE = 4;

    static const event_t INVALID_EVENT      = static_cast<event_t>(-1);

    enum SocketType {
        SOCK_STREAM, // TCP
        SOCK_DGRAM,  // UDP
        SOCK_RAW     // IP
    };

    enum SocketState {
        None,
        Bound,
        Listening,
        Connecting,
        Connected,
        Closed
    };

public:
    static Socket * new_socket(SocketType type, int sd, NetworkManager &nm);

public:
    explicit Socket(int sd, NetworkManager &nm);
    virtual ~Socket();

    virtual SocketType type() = 0;

    int sd() {
        return _sd;
    }

    SocketState state() {
        return _state;
    }

    bool blocking() {
        return _blocking;
    }

    /**
     * Determines whether socket operates in blocking or non-blocking mode.
     *
     * When a socket operates in blocking mode operations on the socket block the caller
     * until a result is present (e.g. a connection request is present in Socket::accept,
     * or data is available in Socket::recv).
     * A socket in non-blocking mode returns Errors::WOULD_BLOCK instead of blocking the caller.
     *
     * Sockets in blocking mode require the application to use a multithreaded workloop.
     *
     * @param blocking whether socket operates in blocking or
     *        non-blocking mode (default = non-blocking)
     */
    void blocking(bool blocking) {
        _blocking = blocking;
    }

    /**
     * Bind socket to <address> and <port>.
     *
     * Only supported by connection-oriented socket types.
     *
     * @param addr the local address to bind to
     * @param port the local port to bind to
     * @return the error code, if any
     */
    virtual Errors::Code bind(IpAddr addr, uint16_t port);

    /**
     * Set socket into listen mode.
     *
     * @return the error code, if any
     */
    virtual Errors::Code listen();

    /**
     * Connect the socket to the socket at <addr>:<port>.
     *
     * @param addr address of the socket to connect to
     * @param port port of the socket to connect to
     * @return the error code, if any
     */
    virtual Errors::Code connect(IpAddr addr, uint16_t port);

    /**
     * Accepts the first connection request from queue of pending connections,
     * and returns a newly created socket in connected state.
     *
     * Beforehand, this socket has been bound to a local address with Socket::bind
     * and is listening for connection requests after Socket::listen.
     *
     * @param socket the accepted socket if no error is indicated
     * @return the error code, if any
     */
    virtual Errors::Code accept(Socket *& socket);

    // TODO: Allow controlled "shutdown" of socket. It must be guarenteed
    //       that all sent data has been transmitted.

    /**
     * Closes and frees the resources associated with the socket.
     *
     * Automatically invoked inside the destructor, if not called manually.
     *
     * @return the error code, if any
     */
    virtual Errors::Code close();

    /**
     * @see Socket::sendto
     *
     * Beforehand, the socket must have been connected to a remote socket.
     */
    ssize_t send(const void *src, size_t amount);

    /**
     * Sends <amount> or a smaller number of bytes (depending on the underlying socket type) from <src>,
     * to the socket at <addr>:<port>.
     *
     * Only connectionless socket types make use of <addr> and <port>.
     *
     * @param src the data to send
     * @param amount the number of bytes to send
     * @param dst_addr destination socket address
     * @param dst_port destination socket port
     * @return the number of sent bytes (<0 = error)
     */
    virtual ssize_t sendto(const void *src, size_t amount, IpAddr dst_addr, uint16_t dst_port) = 0;

    /**
     * @see Socket::recvmsg
     */
    ssize_t recv(void *src, size_t amount);

    /**
     * Receives <amount> or a smaller number of bytes (depending on the underlying socket type) into <dst>.
     *
     * @param dst the destination buffer
     * @param amount the number of bytes to receive
     * @param src_addr if not null, the source address is filled in
     * @param src_port if not null, the source port is filled in
     * @return the number of received bytes (<0 = error)
     */
    virtual ssize_t recvmsg(void *dst, size_t amount, IpAddr *src_addr, uint16_t *src_port) = 0;

protected:
    void fetch_events();
    Errors::Code process_message(NetEventChannel::SocketControlMessage const & message, NetEventChannel::Event &event);
    Errors::Code update_status(Errors::Code err, SocketState state);
    Errors::Code inv_state();
    Errors::Code or_closed(Errors::Code err);

    Errors::Code get_next_data(const uchar *&data, size_t &size);
    void ack_data(size_t size);

    virtual Errors::Code handle_data_transfer(NetEventChannel::DataTransferMessage const & msg);
    virtual Errors::Code handle_ack_data_transfer(NetEventChannel::AckDataTransferMessage const & msg);
    virtual Errors::Code handle_inband_data_transfer(NetEventChannel::InbandDataTransferMessage const & msg, NetEventChannel::Event &event);
    virtual Errors::Code handle_socket_accept(NetEventChannel::SocketAcceptMessage const & msg);
    virtual Errors::Code handle_socket_connected(NetEventChannel::SocketConnectedMessage const & msg);
    virtual Errors::Code handle_socket_closed(NetEventChannel::SocketClosedMessage const & msg);

    void wait_for_event();
    event_t get_wait_event();
    void wait_for_credit();

protected:
    int _sd;
    SocketState _state;
    Errors::Code _close_cause;

    IpAddr _local_addr;
    uint16_t _local_port;
    IpAddr _remote_addr;
    uint16_t _remote_port;

    NetworkManager &_nm;
    Reference<NetEventChannel> _channel;

    bool _blocking;
    event_t _wait_event;
    size_t _waiting;

    DataQueue _recv_queue;
};

}
