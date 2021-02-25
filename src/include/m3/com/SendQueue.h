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

#include <m3/com/SendGate.h>
#include <m3/WorkLoop.h>

namespace m3 {

class SendQueue : public WorkItem {
    struct SendItem : public SListItem {
        explicit SendItem(SendGate &gate, const MsgBuf &msg) noexcept
            : SListItem(),
              gate(gate),
              msg(msg) {
        }

        SendGate &gate;
        MsgBuf msg;
    };

    explicit SendQueue() noexcept : _queue() {
    }

public:
    static SendQueue &get() noexcept {
        return _inst;
    }

    void send(SendGate &gate, const MsgBuf &msg) {
        SendItem *it = new SendItem(gate, msg);
        _queue.append(it);
        if(_queue.length() == 1)
            send_async(*it);
    }
    size_t length() const noexcept {
        return _queue.length();
    }

    virtual void work() override;

private:
    void send_async(SendItem &it);

    SList<SendItem> _queue;
    static SendQueue _inst;
};

}
