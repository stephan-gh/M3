/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <m3/netrs/NetEventChannel.h>

namespace m3 {

class DataQueueRs {
public:
    class Item : public SListItem {
    public:
        Item(NetEventChannelRs::DataMessage const *msg,
             NetEventChannelRs::Event &&event) noexcept;

        IpAddr src_addr() const noexcept;
        port_t src_port() const noexcept;
        const uchar *get_data() const noexcept;
        size_t get_size() const noexcept;
        size_t get_pos() const noexcept;
        void set_pos(size_t pos) noexcept;

    private:
        NetEventChannelRs::DataMessage const *_msg;
        NetEventChannelRs::Event _event;
        size_t _pos;
    };

public:
    ~DataQueueRs();

    void append(Item *item) noexcept;
    bool has_data() const noexcept;
    bool get_next_data(const uchar **data, size_t *size,
                       IpAddr *src_addr, port_t *src_port) noexcept;
    void ack_data(size_t size) noexcept;
    void clear() noexcept;

private:
    SList<Item> _recv_queue;
};

}
