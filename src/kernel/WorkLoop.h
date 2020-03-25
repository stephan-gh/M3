/*
 * Copyright (C) 2016, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/Types.h>

namespace kernel {

class WorkLoop {
public:
    static WorkLoop &get() {
        return _wl;
    }

    explicit WorkLoop() : _run(true) {
    }

    void multithreaded(uint count);

    void run();
    void stop() {
        _run = false;
    }

private:
    static void thread_startup(void *arg);
    void thread_shutdown();

    bool _run;
    static WorkLoop _wl;
};

}
