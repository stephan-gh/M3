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

#include <base/Panic.h>

#include <m3/com/GateStream.h>
#include <m3/com/RecvGate.h>
#include <m3/WorkLoop.h>

#include <thread/ThreadManager.h>

namespace m3 {

WorkItem::~WorkItem() {
    _wl->remove(this);
}

WorkLoop::~WorkLoop() {
#if defined(__gem5__)
    RecvGate::upcall().stop();
#endif
}

void WorkLoop::multithreaded(UNUSED uint count) {
#if defined(__gem5__)
    RecvGate::upcall().start(this, [](GateIStream &is) {
        auto &msg = reinterpret_cast<const KIF::Upcall::DefaultUpcall&>(is.message().data);

        ThreadManager::get().notify(msg.event, &msg, sizeof(msg));

        KIF::DefaultReply reply;
        reply.error = Errors::NONE;
        reply_msg(is, &reply, sizeof(reply));
    });

    for(uint i = 0; i < count; ++i)
        new Thread(thread_startup, this);
#endif
}

void WorkLoop::thread_startup(void *arg) {
    WorkLoop *wl = reinterpret_cast<WorkLoop*>(arg);
    wl->run();

    wl->thread_shutdown();
}

void WorkLoop::thread_shutdown() {
    // first wait until we have no threads left that wait for some event
    ThreadManager &tm = ThreadManager::get();
    while(tm.get().blocked_count() > 0) {
        // we are not interested in the events here; just fetch them before the sleep
        DTU::get().fetch_events();

        DTUIf::sleep();

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
        // wait first to ensure that we check for loop termination *before* going to sleep

        // if there are no events, sleep
        if(DTU::get().fetch_events() == 0)
            DTUIf::sleep();

        tick();

        m3::ThreadManager::get().yield();
    }
}

}
