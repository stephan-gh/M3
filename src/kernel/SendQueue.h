/*
 * Copyright (C) 2016-2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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
#include <base/TCU.h>

namespace kernel {

class SendQueue {
    struct Entry : public m3::SListItem {
        explicit Entry(uint64_t _id, label_t _ident, const void *_msg, size_t _size)
            : SListItem(),
              id(_id),
              ident(_ident),
              msg(_msg),
              size(_size) {
        }

        uint64_t id;
        label_t ident;
        const void *msg;
        size_t size;
    };

public:
    explicit SendQueue(peid_t pe, epid_t ep)
        : _pe(pe),
          _ep(ep),
          _queue(),
          _cur_event(),
          _inflight(0) {
    }
    ~SendQueue();

    int inflight() const {
        return _inflight;
    }
    int pending() const {
        return static_cast<int>(_queue.length());
    }

    event_t send(label_t ident,const void *msg, size_t size, bool onheap);
    void received_reply(const m3::TCU::Message *msg);
    void drop_msgs(label_t ident);
    void abort();

private:
    void send_pending();
    event_t do_send(uint64_t id, label_t ident, const void *msg, size_t size, bool onheap);

    static event_t get_event(uint64_t id);

    peid_t _pe;
    epid_t _ep;
    m3::SList<Entry> _queue;
    event_t _cur_event;
    int _inflight;
    static uint64_t _next_id;
};

}
