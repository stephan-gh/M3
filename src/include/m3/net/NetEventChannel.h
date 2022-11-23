/*
 * Copyright (C) 2021 Nils Asmussen, Barkhausen Institut
 * Copyright (C) 2018, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
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
#include <m3/net/Net.h>

namespace m3 {

class NetEventChannel : public RefCounted {
public:
    static const size_t MSG_SIZE = 2048;
    static const size_t MSG_CREDITS = 4;
    static const size_t MSG_BUF_SIZE = MSG_SIZE * MSG_CREDITS;

    static const size_t REPLY_SIZE = 32;
    static const size_t REPLY_BUF_SIZE = REPLY_SIZE * MSG_CREDITS;

    enum EventType {
        Data,
        Connected,
        Closed,
        CloseReq,
    };

    struct ControlMessage {
        uint64_t type;
    } PACKED;

    struct DataMessage : public ControlMessage {
        uint64_t addr;
        uint64_t port;
        uint64_t size;
        uchar data[0];
    } PACKED;

    struct ConnectedMessage : public ControlMessage {
        uint64_t addr;
        uint64_t port;
    } PACKED;

    struct ClosedMessage : public ControlMessage {
    } PACKED;

    struct CloseReqMessage : public ControlMessage {
    } PACKED;

    static const size_t MAX_PACKET_SIZE =
        MSG_SIZE - (sizeof(DataMessage) + sizeof(TCU::Message::Header));

    class Event {
        friend class NetEventChannel;

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
        explicit Event(const TCU::Message *msg, NetEventChannel *channel) noexcept;

        const TCU::Message *_msg;
        NetEventChannel *_channel;
        bool _ack;
    };

    NetEventChannel(capsel_t caps);

    Errors::Code build_data_message(void *buffer, size_t buf_size, const Endpoint &ep,
                                    const void *payload, size_t payload_size);
    Errors::Code send_data(const void *buffer, size_t payload_size);
    bool send_close_req();

    bool can_send() const noexcept;
    bool has_events() noexcept;
    bool has_all_credits();
    Event recv_message();

    void wait_for_events();
    void wait_for_credits();

    void fetch_replies();

private:
    RecvGate _rgate;
    RecvGate _rplgate;
    SendGate _sgate;
};

}
