/*
 * Copyright (C) 2019, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
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

#include <base/col/SList.h>

#include <m3/net/NetEventChannel.h>

namespace m3 {

class DataQueue {
public:
    class Item : public SListItem {
    public:
        Item(NetEventChannel::InbandDataTransferMessage const *msg,
             NetEventChannel::Event &&event) noexcept;

        const uchar *get_data() noexcept;
        size_t get_size() noexcept;
        size_t get_pos() noexcept;
        void set_pos(size_t pos) noexcept;

    private:
        NetEventChannel::InbandDataTransferMessage const *_msg;
        NetEventChannel::Event _event;
        size_t _pos;
    };

public:
    ~DataQueue();

    void append(Item *item) noexcept;
    bool has_data() noexcept;
    bool get_next_data(const uchar *&data, size_t &size) noexcept;
    void ack_data(size_t size) noexcept;
    void clear() noexcept;

private:
    SList<Item> _recv_queue;
};

}

