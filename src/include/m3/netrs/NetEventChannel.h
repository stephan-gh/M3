/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/TCU.h>
#include <base/util/Reference.h>

#include <m3/com/GateStream.h>
#include <m3/com/MemGate.h>
#include <m3/com/RecvGate.h>
#include <m3/com/SendGate.h>
#include <m3/netrs/Net.h>

namespace m3 {

class NetEventChannelRs : public RefCounted {
public:
    static const size_t MSG_SIZE                = 2048;
    static const size_t MSG_CREDITS             = 4;
    static const size_t MSG_BUF_SIZE            = MSG_SIZE * MSG_CREDITS;

    static const size_t REPLY_SIZE              = 32;
    static const size_t REPLY_BUF_SIZE          = REPLY_SIZE * MSG_CREDITS;

    enum EventType {
        Data,
        Connected,
        Closed,
        CloseReq,
    };

    struct ControlMessage {
        uint64_t type;
    } PACKED;

    struct SocketControlMessage : public ControlMessage {
        uint64_t sd;
    } PACKED;

    struct DataMessage : public SocketControlMessage {
        uint64_t addr;
        uint64_t port;
        uint64_t size;
        uchar data[0];
    } PACKED;

    struct ConnectedMessage : public SocketControlMessage {
        uint64_t addr;
        uint64_t port;
    } PACKED;

    struct ClosedMessage : public SocketControlMessage {
    } PACKED;

    struct CloseReqMessage : public SocketControlMessage {
    } PACKED;

    class Event {
    friend class NetEventChannelRs;
    public:
        Event() noexcept;
        ~Event();

        Event(const Event &e) = delete;
        Event(Event &&e) noexcept;
        Event &operator=(const Event &e) = delete;
        Event &operator=(Event &&e) noexcept;

        bool is_present() noexcept;
        void finish();

        const ControlMessage *get_message() noexcept;
    private:
        explicit Event(const TCU::Message *msg, NetEventChannelRs *channel) noexcept;

        const TCU::Message *_msg;
        NetEventChannelRs *_channel;
        bool _ack;
    };

    NetEventChannelRs(capsel_t caps);

    bool send_data(int sd, IpAddr addr, uint16_t port, size_t size, std::function<void(uchar *)> cb_data);
    bool send_close_req(int sd);

    bool can_send() const;
    bool has_events() const;
    Event recv_message();

    void fetch_replies();

private:
    RecvGate _rgate;
    RecvGate _rplgate;
    SendGate _sgate;
};

}
