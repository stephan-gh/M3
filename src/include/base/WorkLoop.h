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
#include <base/DTU.h>

#include <functional>

namespace m3 {

class WorkLoop;

class WorkItem : public SListItem {
    friend class WorkLoop;
public:
    virtual ~WorkItem() {
    }

    virtual void work() = 0;
};

class WorkLoop {
    static const size_t MAX_ITEMS   = 32;

public:
    explicit WorkLoop() : _changed(false), _permanents(0), _count(), _items(), _sleep_handler(nullptr) {
    }
    virtual ~WorkLoop() {
    }

    bool has_items() const {
        return _count > _permanents;
    }

    void add(WorkItem *item, bool permanent);
    void remove(WorkItem *item);

    virtual void multithreaded(uint count) = 0;

    void tick();
    virtual void run();
    void stop() {
        _permanents = _count;
    }

    // TODO: Allow multiple sleep handlers?
    using sleep_handler_t = std::function<void()>;
    void set_sleep_handler(sleep_handler_t sleep_handler);

protected:
    static void thread_startup(void *);
    virtual void thread_shutdown();

private:
    bool _changed;
    size_t _permanents;
    size_t _count;
    WorkItem *_items[MAX_ITEMS];
    sleep_handler_t _sleep_handler;
};

}
