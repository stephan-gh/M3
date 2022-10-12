/*
 * Copyright (C) 2016-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Copyright (C) 2016, Matthias Hille <matthias.hille@tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2020 Nils Asmussen, Barkhausen Institut
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

#include <base/Panic.h>
#include <base/log/Lib.h>

#include <thread/Thread.h>

namespace m3 {

extern "C" void thread_switch(Regs *o, Regs *n);

class ThreadManager {
    friend class Thread;

public:
    static ThreadManager &get() {
        return inst;
    }

    Thread *current() {
        return _current;
    }
    size_t thread_count() const {
        return _ready.length() + _blocked.length() + _sleep.length();
    }
    size_t ready_count() const {
        return _ready.length();
    }
    size_t blocked_count() const {
        return _blocked.length();
    }
    size_t sleeping_count() const {
        return _sleep.length();
    }
    const unsigned char *get_current_msg() const {
        return _current->get_msg();
    }

    event_t get_wait_event() {
        // if we have no other threads available, don't use events
        if(sleeping_count() == 0)
            return 0;
        // otherwise, use a unique number
        return _next_id++;
    }

    void init(uint threads);

    void wait_for(event_t event) {
        // TODO: Maybe a bug, there could be threads in _ready.
        if(_sleep.length() == 0)
            panic("Not enough threads"_cf);
        _current->subscribe(event);
        _blocked.append(_current);
        LLOG(THREAD, "Thread {} waits for {:x}"_cf, _current->id(), event);
        if(_ready.length())
            switch_to(_ready.remove_first());
        else
            switch_to(_sleep.remove_first());
    }

    void yield() {
        if(_ready.length()) {
            // prepend the thread to the list to prefer the reuse of threads
            _sleep.insert(nullptr, _current);
            switch_to(_ready.remove_first());
        }
    }

    void notify(event_t event, const void *msg = nullptr, size_t size = 0) {
        assert(size <= Thread::MAX_MSG_SIZE);
        for(auto it = _blocked.begin(); it != _blocked.end();) {
            auto old = it++;
            if(old->trigger_event(event)) {
                Thread *t = &(*old);
                t->set_msg(msg, size);
                LLOG(THREAD, "Waking up thread {} for event {:x}"_cf, t->id(), event);
                _blocked.remove(t);
                _ready.append(t);
            }
        }
    }

    void stop() {
        LLOG(THREAD, "Stopping thread {}"_cf, _current->id());
        if(_ready.length())
            switch_to(_ready.remove_first());
        if(_sleep.length())
            switch_to(_sleep.remove_first());
    }

private:
    explicit ThreadManager() : _current(), _ready(), _blocked(), _sleep(), _next_id(1) {
        _current = new Thread();
    }

    void add(Thread *t) {
        _sleep.append(t);
    }
    void remove(Thread *t) {
        _ready.remove(t);
        _blocked.remove(t);
        _sleep.remove(t);
    }

    void switch_to(Thread *t) {
        LLOG(THREAD, "Switching from {} to {}"_cf, _current->id(), t->id());
        auto old = _current;
        _current = t;
        thread_switch(&old->_regs, &t->_regs);
    }

    Thread *_current;
    m3::SList<Thread> _ready;
    m3::SList<Thread> _blocked;
    m3::SList<Thread> _sleep;
    event_t _next_id;
    static ThreadManager inst;
};

}