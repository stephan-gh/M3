/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/TCU.h>

#include <functional>

namespace m3 {

class WorkLoop;

class WorkItem {
    friend class WorkLoop;

public:
    virtual ~WorkItem();

    virtual void work() = 0;

private:
    WorkLoop *_wl;
};

class WorkLoop {
    static const size_t MAX_ITEMS = 32;

public:
    explicit WorkLoop() noexcept : _permanents(0), _count(), _items() {
    }
    ~WorkLoop();

    bool has_items() const noexcept {
        return _count > _permanents;
    }

    void add(WorkItem *item, bool permanent);
    void remove(WorkItem *item);

    void multithreaded(uint count);

    void tick();
    void run();

    void stop() noexcept {
        _permanents = _count;
    }

private:
    static void thread_startup(void *);
    void thread_shutdown();

    size_t _permanents;
    size_t _count;
    WorkItem *_items[MAX_ITEMS];
};

}
