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
#include <m3/tiles/Activity.h>

using namespace m3;

static const TimeDuration interval = TimeDuration::from_millis(20);
static Server<EventHandler<>> *server;
static TimeInstant next_tick = TimeInstant::now();

struct TickWorkItem : public WorkItem {
    void work() override {
        auto cur = TimeInstant::now();
        if(cur >= next_tick) {
            SLOG(TIMER, "Timer tick @ " << cur.as_nanos());
            server->handler()->broadcast(0);
            next_tick = TimeInstant::now() + interval;
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
        auto now = TimeInstant::now();
        if(now > next_tick)
            Activity::sleep_for(next_tick.duration_since(now));

        wl.tick();
    }

    delete server;
    return 0;
}
