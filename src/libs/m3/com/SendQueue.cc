/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2021 Nils Asmussen, Barkhausen Institut
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

#include <base/Init.h>
#include <base/TCU.h>
#include <base/log/Lib.h>

#include <m3/com/SendQueue.h>

namespace m3 {

INIT_PRIO_SENDQUEUE SendQueue SendQueue::_inst;

void SendQueue::work() {
    if(_queue.length() > 0) {
        SendItem *it = _queue.remove_first();
        LLOG(IPC, "Removing {} from queue"_cf, static_cast<void *>(it));
        delete it;
        if(_queue.length() > 0) {
            SendItem &first = *_queue.begin();
            first.gate.send(first.msg);
            LLOG(IPC, "Sending {} from queue"_cf, static_cast<void *>(&first));
        }
    }
}

void SendQueue::send_async(SendItem &it) {
    it.gate.send(it.msg);
    LLOG(IPC, "Sending {} from queue"_cf, static_cast<void *>(&it));
}

}
