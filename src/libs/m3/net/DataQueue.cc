/*
 * Copyright (C) 2019-2021 Nils Asmussen, Barkhausen Institut
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

#include <m3/net/DataQueue.h>

namespace m3 {

DataQueue::Item::Item(NetEventChannel::DataMessage const *msg,
                      NetEventChannel::Event &&event) noexcept
    : _msg(msg), _event(std::move(event)), _pos(0) {
}

IpAddr DataQueue::Item::src_addr() const noexcept {
    return IpAddr(_msg->addr);
}

port_t DataQueue::Item::src_port() const noexcept {
    return _msg->port;
}

const uchar *DataQueue::Item::get_data() const noexcept {
    return _msg->data;
}

size_t DataQueue::Item::get_size() const noexcept {
    return _msg->size;
}

size_t DataQueue::Item::get_pos() const noexcept {
    return _pos;
}

void DataQueue::Item::set_pos(size_t pos) noexcept {
    assert(pos <= get_size());
    _pos = pos;
}

DataQueue::~DataQueue() {
    clear();
}

void DataQueue::append(Item *item) noexcept {
    _recv_queue.append(item);
}

bool DataQueue::has_data() const noexcept {
    return _recv_queue.length() > 0;
}

bool DataQueue::get_next_data(const uchar **data, size_t *size, Endpoint *ep) noexcept {
    if(!has_data())
        return false;

    Item &item = *_recv_queue.begin();
    *data = item.get_data() + item.get_pos();
    *size = item.get_size() - item.get_pos();
    if(ep)
        *ep = Endpoint(item.src_addr(), item.src_port());
    return true;
}

void DataQueue::ack_data(size_t size) noexcept {
    // May be called exactly once for every successful invocation of get_next_data().
    assert(_recv_queue.length() > 0);

    Item &item = *_recv_queue.begin();
    assert(item.get_pos() + size <= item.get_size());
    item.set_pos(item.get_pos() + size);

    // Remove item if its data is exhausted
    if(item.get_pos() >= item.get_size())
        delete _recv_queue.remove_first();
}

void DataQueue::clear() noexcept {
    Item *item;
    while((item = _recv_queue.remove_first()) != nullptr)
        delete item;
}

}
