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

#include "lwip/tcp.h"
#include "lwip/raw.h"

#include "socket/LwipUdpSocket.h"
#include "socket/LwipTcpSocket.h"
#include "socket/LwipRawSocket.h"

#include "SocketSession.h"
#include "FileSession.h"

using namespace m3;

void NetEventChannelWorkItem::work() {
    size_t maxSendCount = SocketSession::MAX_SEND_RECEIVE_BATCH_SIZE;
    while(maxSendCount--) {
        auto message = _channel.recv_message();
        if(!message.is_present())
            return;

        auto msg = message.get_message();
        switch(msg->type) {
            case NetEventChannel::InbandDataTransfer:
            {
                auto data_msg = static_cast<const NetEventChannel::InbandDataTransferMessage *>(msg);
                LwipSocket * socket = _session.get_socket(data_msg->sd);
                if(!socket) {
                    LOG_SESSION(&_session, "NetEventChannel::recv_message failed: invalid socket descriptor" << data_msg->sd);
                    break;
                }

                ssize_t sent_size = socket->send_data(data_msg->data, data_msg->size);
                if(sent_size < static_cast<ssize_t>(data_msg->size)) {
                    size_t ack_size = static_cast<size_t>(Math::max<ssize_t>(sent_size, 0));
                    auto item = new DataQueue::Item(data_msg, std::move(message));
                    item->set_pos(ack_size);
                    socket->enqueue_data(item);
                }

                break;
            }
            default:
                LOG_SESSION(&_session, "NetEventChannel::recv_message: unsupported message type " << msg->type);
        }
    }
}

SocketSession::SocketSession(m3::WorkLoop *wl, capsel_t srv_sel, m3::RecvGate& rgate)
    : NMSession(srv_sel),
      _wl(wl),
      _sgate(nullptr),
      _rgate(rgate),
      _channel_caps(ObjCap::INVALID),
      _channel(nullptr),
      _channelWorkItem(nullptr),
      sockets() {
}

SocketSession::~SocketSession() {
    for(size_t i = 0; i < MAX_SOCKETS; i++)
        release_sd(static_cast<int>(i));

    if(_channelWorkItem)
        delete _channelWorkItem;

    delete _sgate;
    delete _channel;

    if(_channel_caps != ObjCap::INVALID) {
        VPE::self().revoke(KIF::CapRngDesc(KIF::CapRngDesc::OBJ, _channel_caps, 6));
    }
}

m3::Errors::Code SocketSession::obtain(capsel_t srv_sel, m3::KIF::Service::ExchangeData& data) {
    if(data.caps == 1) {
        return get_sgate(data);
    } else if(data.caps == 3) {
        return establish_channel(data);
    } else if(data.caps == 2 && data.args.count == 4) {
        return open_file(srv_sel, data);
    } else {
        return Errors::INV_ARGS;
    }
}

m3::Errors::Code SocketSession::get_sgate(m3::KIF::Service::ExchangeData& data) {
    if(_sgate)
        return Errors::INV_ARGS;

    label_t label = ptr_to_label(this);
    _sgate = new SendGate(SendGate::create(&_rgate, SendGateArgs().label(label)
                                                                  .credits(1)));

    data.caps = KIF::CapRngDesc(KIF::CapRngDesc::OBJ, _sgate->sel()).value();
    return Errors::NONE;
}

m3::Errors::Code SocketSession::establish_channel(m3::KIF::Service::ExchangeData& data) {
    if(data.caps == 3) {
        if(_channel_caps != ObjCap::INVALID) {
            LOG_SESSION(this, "handle_obtain failed: data channel is already established");
            return Errors::INV_ARGS;
        }

        // 0 - 2: Server
        // 3 - 5: Client
        _channel_caps = VPE::self().alloc_sels(6);
        NetEventChannel::prepare_caps(_channel_caps, NetEventChannel::BUFFER_SIZE);
        _channel = new NetEventChannel(_channel_caps, true);

        _channelWorkItem = new NetEventChannelWorkItem(*_channel, *this);
        _wl->add(_channelWorkItem, false);

        // TODO: pass size as argument
        KIF::CapRngDesc crd(KIF::CapRngDesc::OBJ, _channel_caps + 3, 3);
        data.caps = crd.value();
        data.args.count = 0;
        return Errors::NONE;
    }
    else
        return Errors::INV_ARGS;
}

m3::Errors::Code SocketSession::open_file(capsel_t srv_sel, m3::KIF::Service::ExchangeData& data) {
    if(data.caps != 2 || data.args.count != 4)
        return Errors::INV_ARGS;

    int sd = data.args.vals[0];
    LwipSocket *socket = get_socket(sd);
    if(socket) {
        int mode = data.args.vals[1];
        if(!(mode & FILE_RW)) {
            LOG_SESSION(this, "open_file failed: invalid mode");
            return Errors::INV_ARGS;
        }

        if((socket->_rfile && (mode & FILE_R)) || (socket->_sfile && (mode & FILE_W))) {
            LOG_SESSION(this, "open_file failed: socket already has a file session attached");
            return Errors::INV_ARGS;
        }

        size_t rmemsize = data.args.vals[2];
        size_t smemsize = data.args.vals[3];
        FileSession *file = new FileSession(_wl, srv_sel, socket, mode, rmemsize, smemsize);
        if(file->is_recv())
            socket->_rfile = file;
        if(file->is_send())
            socket->_sfile = file;
        socket->_rgate = &_rgate;
        data.args.count = 0;
        data.caps = file->caps().value();
        LOG_SESSION(this, "open_file: " << sd << "@" << (file->is_recv() ? "r" : "") << (file->is_send() ? "s" : ""));
        return Errors::NONE;
    } else {
        LOG_SESSION(this, "open_file failed: invalid socket descriptor");
        return Errors::INV_ARGS;
    }
}

void SocketSession::create(m3::GateIStream& is) {
    Socket::SocketType type;
    uint8_t protocol;
    is >> type >> protocol;
    LOG_SESSION(this, "net::create(type=" << type << ", protocol=" << protocol << ")");

    if(_channel == nullptr) {
        LOG_SESSION(this, "create failed: no channel has been established");
        reply_error(is, Errors::INV_STATE);
        return;
    }

    LwipSocket *socket;
    switch(type) {
        case Socket::SOCK_STREAM:
            socket = new LwipTcpSocket(_wl, this);
            break;
        case Socket::SOCK_DGRAM:
            socket = new LwipUdpSocket(this);
            break;
        case Socket::SOCK_RAW:
            socket = new LwipRawSocket(this);
            break;
        default:
            LOG_SESSION(this, "create failed: invalid socket type");
            reply_error(is, Errors::INV_ARGS);
            return;
    }
    socket->channel(_channel);

    Errors::Code err = socket->create(protocol);
    if(err != Errors::NONE) {
        reply_error(is, err);
        return;
    }

    // allocate new socket descriptor
    int sd = this->request_sd(socket);
    if(sd == -1) {
        delete socket;
        LOG_SESSION(this, "create failed: maximum number of sockets reached");
        reply_error(is, Errors::NO_SPACE);
        return;
    }

    LOG_SESSION(this, "-> sd=" << sd);
    reply_vmsg(is, Errors::NONE, sd);
}

void SocketSession::bind(m3::GateIStream& is) {
    int sd;
    uint32_t addr;
    uint16_t port;
    is >> sd >> addr >> port;
    ip4_addr ip_addr = IPADDR4_INIT(lwip_htonl(addr));
    LOG_SESSION(this, "net::bind(sd=" << sd << ", addr=" << ip4addr_ntoa(&ip_addr) << ", port=" << port << ")");

    LwipSocket *socket = get_socket(sd);
    if(socket) {
        reply_error(is, socket->bind(ip_addr, port));
    } else {
        LOG_SESSION(this, "bind failed: invalid socket descriptor");
        reply_error(is, Errors::INV_ARGS);
    }

}

void SocketSession::listen(m3::GateIStream& is) {
    int sd;
    is >> sd;
    LOG_SESSION(this, "net::listen(sd=" << sd << ")");

    LwipSocket *socket = get_socket(sd);
    if(socket) {
        reply_error(is, socket->listen());
    } else {
        LOG_SESSION(this, "listen failed: invalid socket descriptor");
        reply_error(is, Errors::INV_ARGS);
    }
}

void SocketSession::connect(m3::GateIStream& is) {
    int sd;
    uint32_t addr;
    uint16_t port;
    is >> sd >> addr >> port;
    ip4_addr ip_addr = IPADDR4_INIT(lwip_htonl(addr));
    LOG_SESSION(this, "net::connect(sd=" << sd << ", addr=" << ip4addr_ntoa(&ip_addr)
        << ", port=" << port << ")");

    LwipSocket *socket = get_socket(sd);
    if(socket) {
        reply_error(is, socket->connect(ip_addr, port));
    } else {
        LOG_SESSION(this, "connect failed: invalid socket descriptor");
        reply_error(is, Errors::INV_ARGS);
    }
}

void SocketSession::close(m3::GateIStream& is) {
    int sd;
    is >> sd;
    LOG_SESSION(this, "net::close(sd=" << sd << ")");

    LwipSocket *socket = get_socket(sd);
    if(socket) {
        Errors::Code err = socket->close();
        release_sd(sd);
        reply_error(is, err);
    } else {
        LOG_SESSION(this, "close failed: invalid socket descriptor");
        reply_error(is, Errors::INV_ARGS);
    }
}

LwipSocket * SocketSession::get_socket(int sd) {
    if(sd >= 0 && static_cast<size_t>(sd) < MAX_SOCKETS)
        return sockets[sd];
    return nullptr;
}

int SocketSession::request_sd(LwipSocket *socket) {
    for(size_t i = 0; i < MAX_SOCKETS; i++) {
        if(sockets[i] == nullptr) {
            sockets[i] = socket;
            socket->set_sd(static_cast<int>(i));
            return socket->sd();
        }
    }
    return -1;
}

void SocketSession::release_sd(int sd) {
    // TODO: How to prevent accidental use of different socket through an reused socket descriptor?
    // Maybe embed a counter into the upper bits of the socket descriptor?
    if(sd >= 0 && static_cast<size_t>(sd) < MAX_SOCKETS && sockets[sd] != nullptr) {
        // TODO: Free lwip resources?
        delete sockets[sd];
        sockets[sd] = nullptr;
    }
}
