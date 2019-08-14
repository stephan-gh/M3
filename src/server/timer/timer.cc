/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/log/Services.h>

#include <m3/server/Server.h>
#include <m3/server/EventHandler.h>

using namespace m3;

static const cycles_t interval = 20000000;
static Server<EventHandler<>> *server;
static cycles_t next_tick = 0;

struct TickWorkItem : public WorkItem {
    void work() override {
        cycles_t tsc = DTU::get().tsc();
        if(tsc >= next_tick) {
            SLOG(TIMER, "Timer tick @ " << tsc);
            server->handler()->broadcast(0);
            next_tick = DTU::get().tsc() + interval;
        }
    }
};

int main() {
    WorkLoop wl;

    server = new Server<EventHandler<>>("timer", &wl, std::make_unique<EventHandler<>>());

    TickWorkItem wi;
    wi.work();

    wl.add(&wi, true);

    while(wl.has_items()) {
        DTU::get().fetch_events();

        DTUIf::sleep_for(next_tick - DTU::get().tsc());

        wl.tick();
    }

    delete server;
    return 0;
}
