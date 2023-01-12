/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

#include <base/Panic.h>

#include <m3/WorkLoop.h>
#include <m3/com/GateStream.h>
#include <m3/com/RecvGate.h>
#include <m3/tiles/OwnActivity.h>

#include <thread/ThreadManager.h>

namespace m3 {

WorkItem::~WorkItem() {
    _wl->remove(this);
}

WorkLoop::~WorkLoop() {
    RecvGate::upcall().stop();
}

void WorkLoop::multithreaded(UNUSED uint count) {
    RecvGate::upcall().start(this, [](GateIStream &is) {
        auto &msg = reinterpret_cast<const KIF::Upcall::DefaultUpcall &>(is.message().data);

        ThreadManager::get().notify(msg.event, &msg, sizeof(msg));

        MsgBuf reply_buf;
        auto &reply = reply_buf.cast<KIF::DefaultReply>();
        reply.error = Errors::SUCCESS;
        reply_msg(is, reply_buf);
    });

    for(uint i = 0; i < count; ++i)
        new Thread(thread_startup, this);
}

void WorkLoop::thread_startup(void *arg) {
    WorkLoop *wl = reinterpret_cast<WorkLoop *>(arg);
    wl->run();

    wl->thread_shutdown();
}

void WorkLoop::thread_shutdown() {
    // first wait until we have no threads left that wait for some event
    ThreadManager &tm = ThreadManager::get();
    while(tm.get().blocked_count() > 0) {
        OwnActivity::sleep();

        tick();

        tm.yield();
    }

    tm.stop();

    // just in case there is no ready thread
    exit(1);
}

void WorkLoop::add(WorkItem *item, bool permanent) {
    assert(_count < MAX_ITEMS);
    item->_wl = this;
    _items[_count++] = item;
    if(permanent)
        _permanents++;
}

void WorkLoop::remove(WorkItem *item) {
    for(size_t i = 0; i < MAX_ITEMS; ++i) {
        if(_items[i] == item) {
            _items[i] = nullptr;
            for(++i; i < MAX_ITEMS; ++i)
                _items[i - 1] = _items[i];
            _count--;
            break;
        }
    }
}

void WorkLoop::tick() {
    for(size_t i = 0; i < _count; ++i)
        _items[i]->work();
}

void WorkLoop::run() {
    while(has_items()) {
        OwnActivity::sleep();

        tick();

        m3::ThreadManager::get().yield();
    }
}

}
