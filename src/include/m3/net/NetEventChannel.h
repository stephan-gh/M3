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

#include <base/DTU.h>
#include <base/util/Reference.h>

#include <m3/com/GateStream.h>
#include <m3/com/MemGate.h>
#include <m3/com/RecvGate.h>
#include <m3/com/SendGate.h>
#include <m3/net/Net.h>

namespace m3 {

class NetEventChannel : public RefCounted {
public:
    static const size_t BUFFER_SIZE             = 2 * 1024 * 1024;

    static const size_t MSG_SIZE                = 2048;
    static const size_t MSG_BUF_SIZE            = MSG_SIZE * 4;
    static const size_t MSG_CREDITS             = MSG_BUF_SIZE;

    static const size_t INBAND_DATA_SIZE        = 2048;
    static const size_t INBAND_DATA_BUF_SIZE    = INBAND_DATA_SIZE * 4;
    static const size_t INBAND_DATA_CREDITS     = INBAND_DATA_BUF_SIZE;

    enum ControlMessageType {
        DataTransfer,
        AckDataTransfer,
        InbandDataTransfer,
        SocketAccept,
        AckSocketAccept,
        SocketConnected,
        SocketClosed,
    };

    struct ControlMessage {
        ControlMessageType type;
    };

    struct SocketControlMessage : public ControlMessage {
        int sd;
    };

    struct DataTransferMessage : public SocketControlMessage {
        size_t pos;
        size_t size;
    };

    struct AckDataTransferMessage : public SocketControlMessage {
        size_t pos;
        size_t size;
    };

    struct InbandDataTransferMessage : public SocketControlMessage {
        size_t size;
        uchar data[0];
    };

    struct SocketAcceptMessage : public SocketControlMessage {
        int new_sd;
        IpAddr remote_addr;
        uint16_t remote_port;
    };

    struct SocketConnectedMessage : public SocketControlMessage {
    };

    struct SocketClosedMessage : public SocketControlMessage {
        Errors::Code cause;
    };

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

        GateIStream to_stream() noexcept;
        const ControlMessage *get_message() noexcept;
    private:
        explicit Event(const DTU::Message *msg, NetEventChannel *channel) noexcept;

        const DTU::Message *_msg;
        NetEventChannel *_channel;
        bool _ack;
    };

    class EventWorkItem : public WorkItem {
    public:
        explicit EventWorkItem(NetEventChannel *channel) noexcept : _channel(channel) {
        }

        virtual void work() override;

    protected:
        NetEventChannel *_channel;
    };

    /**
     * caps + 0: _rgate_srv, receives messages from _sgate_cli, and replies for _sgate_srv
     * caps + 1: _sgate_srv, send messages to _rgate_cli, receives replies via _rgate_srv, has unlimited credits
     * caps + 2: _mem_srv, global memory with a size of `2 * size`
     * caps + 3: _rgate_cli, receives messages from _sgate_srv, and replies for _sgate_cli
     * caps + 4: _sgate_cli, send messages to _rgate_srv, receives replies via _rgate_cli
     * caps + 5: _mem_cli, global memory with a size of `2 * size`, derived from _mem_srv
     */
    static void prepare_caps(capsel_t caps, size_t size);

    NetEventChannel(capsel_t caps, bool use_credits) noexcept;

    void data_transfer(int sd, size_t pos, size_t size);
    void ack_data_transfer(int sd, size_t pos, size_t size);
    bool inband_data_transfer(int sd, size_t size, std::function<void(uchar *)> cb_data);
    void socket_accept(int sd, int new_sd, IpAddr remote_addr, uint16_t remote_port);
    void socket_connected(int sd);
    void socket_closed(int sd, Errors::Code cause);

    using evhandler_t = std::function<void(Event& event)>;
    using crdhandler_t = std::function<void(event_t wait_event, size_t waiting)>;

    /**
     * Starts to listen for received events and credits, i.e., adds an item to the given WorkLoop.
     *
     * @param wl the workloop
     * @param evhandler the handler to call for received events
     * @param crdhandler the handler to call when received credits
     */
    void start(WorkLoop *wl, evhandler_t evhandler, crdhandler_t crdhandler);

    /**
     * Stops to listen for received events
     */
    void stop();

    Event recv_message();

    bool has_events(evhandler_t &evhandler, crdhandler_t &crdhandler);

    bool has_credits() noexcept;
    void set_credit_event(event_t event) noexcept;
    event_t get_credit_event() noexcept;
    void wait_for_credit() noexcept;

private:
    void send_message(const void* msg, size_t size);

    bool _ret_credits;
    RecvGate _rgate;
    SendGate _sgate;
    std::unique_ptr<EventWorkItem> _workitem;
    evhandler_t _evhandler;
    crdhandler_t _crdhandler;
    event_t _credit_event;
    // Number of entites waiting for credits on _sgate.
    size_t _waiting_credit;
};

}
