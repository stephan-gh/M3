/*
 * Copyright (C) 2019-2021 Nils Asmussen, Barkhausen Institut
 * Copyright (C) 2019, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
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

#include <base/col/SList.h>
#include <base/util/Option.h>

#include <m3/net/NetEventChannel.h>

#include <optional>
#include <tuple>

namespace m3 {

class DataQueue {
public:
    class Item : public SListItem {
    public:
        Item(NetEventChannel::DataMessage const *msg, NetEventChannel::Event &&event) noexcept;

        IpAddr src_addr() const noexcept;
        port_t src_port() const noexcept;
        const uchar *get_data() const noexcept;
        size_t get_size() const noexcept;
        size_t get_pos() const noexcept;
        void set_pos(size_t pos) noexcept;

    private:
        NetEventChannel::DataMessage const *_msg;
        NetEventChannel::Event _event;
        size_t _pos;
    };

public:
    ~DataQueue();

    void append(Item *item) noexcept;
    bool has_data() const noexcept;
    Option<std::tuple<const uchar *, size_t, Endpoint>> get_next_data() noexcept;
    void ack_data(size_t size) noexcept;
    void clear() noexcept;

private:
    SList<Item> _recv_queue;
};

}
