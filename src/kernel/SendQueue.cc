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

#include <base/log/Kernel.h>

#include "pes/VPE.h"
#include "DTU.h"
#include "SendQueue.h"
#include "SyscallHandler.h"

namespace kernel {

uint64_t SendQueue::_next_id = 0;

SendQueue::~SendQueue() {
    // ensure that there are no messages left for this SendQueue in the receive buffer
    m3::DTU::get().drop_msgs(SyscallHandler::srvep(), m3::ptr_to_label(this));
}

event_t SendQueue::get_event(uint64_t id) {
    return static_cast<event_t>(1) << (sizeof(event_t) * 8 - 1) | id;
}

event_t SendQueue::send(epid_t dst_ep, label_t ident, const void *msg, size_t size, bool onheap) {
    KLOG(SQUEUE, "SendQueue[" << _desc.id << "]: trying to send message");

    if(_inflight == -1)
        return 0;

    if(_inflight == 0)
        return do_send(dst_ep, _next_id++, msg, size, onheap);

    // if it's not already on the heap, put it there
    if(!onheap) {
        void *nmsg = malloc(size);
        memcpy(nmsg, msg, size);
        msg = nmsg;
    }

    KLOG(SQUEUE, "SendQueue[" << _desc.id << "]: queuing message");

    Entry *e = new Entry(_next_id++, dst_ep, ident, msg, size);
    _queue.append(e);
    return get_event(e->id);
}

void SendQueue::send_pending() {
    if(_queue.length() == 0)
        return;

    Entry *e = _queue.remove_first();

    KLOG(SQUEUE, "SendQueue[" << _desc.id << "]: found pending message");

    // it might happen that there is another message in flight now
    if(_inflight != 0) {
        KLOG(SQUEUE, "SendQueue[" << _desc.id << "]: queuing message");
        _queue.append(e);
        return;
    }

    // pending messages have always been copied to the heap
    do_send(e->dst_ep, e->id, e->msg, e->size, true);
    delete e;
}

void SendQueue::received_reply(epid_t ep, const m3::DTU::Message *msg) {
    KLOG(SQUEUE, "SendQueue[" << _desc.id << "]: received reply");

    m3::ThreadManager::get().notify(_cur_event, msg, msg->length + sizeof(m3::DTU::Message::Header));

    // now that we've copied the message, we can mark it read
    m3::DTU::get().mark_read(ep, msg);

    if(_inflight != -1) {
        assert(_inflight > 0);
        _inflight--;

        send_pending();
    }
}

event_t SendQueue::do_send(epid_t dst_ep, uint64_t id, const void *msg, size_t size, bool onheap) {
    KLOG(SQUEUE, "SendQueue[" << _desc.id << "]: sending message");

    _cur_event = get_event(id);
    _inflight++;

    if(DTU::get().send_to(_desc, dst_ep, 0, msg, size, m3::ptr_to_label(this),
                          SyscallHandler::srvep()) != m3::Errors::NONE) {
        PANIC("send failed");
    }

    if(onheap)
        free(const_cast<void*>(msg));
    return _cur_event;
}

void SendQueue::drop_msgs(label_t ident) {
    size_t n = 0;
    Entry *prev = nullptr;
    for(auto it = _queue.begin(); it != _queue.end(); ) {
        auto old = it++;
        if(old->ident == ident) {
            _queue.remove(prev, &*old);
            free(const_cast<void*>(old->msg));
            delete &*old;
            n++;
        }
        else
            prev = &*old;
    }

    KLOG(SQUEUE, "SendQueue[" << _desc.id << "]: dropped " << n << " msgs for " << m3::fmt(ident, "p"));
}

void SendQueue::abort() {
    KLOG(SQUEUE, "SendQueue[" << _desc.id << "]: aborting");

    if(_inflight)
        m3::ThreadManager::get().notify(_cur_event);
    _inflight = -1;

    while(_queue.length() > 0)
        delete _queue.remove_first();
}

}
