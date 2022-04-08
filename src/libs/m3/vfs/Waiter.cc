/*
 * Copyright (C) 2022 Nils Asmussen, Barkhausen Institut
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

#include <m3/tiles/OwnActivity.h>
#include <m3/vfs/File.h>
#include <m3/vfs/FileTable.h>
#include <m3/vfs/Waiter.h>

namespace m3 {

void FileWaiter::wait(uint events) {
    while(true) {
        if(tick_sockets(events))
            break;

        Activity::sleep();
    }
}

void FileWaiter::wait_for(TimeDuration timeout, uint dirs) {
    auto end = TimeInstant::now() + timeout;
    auto now = TimeInstant::now();
    while(now < end) {
        if(tick_sockets(dirs))
            break;

        Activity::sleep_for(end.duration_since(now));
        now = TimeInstant::now();
    }
}

bool FileWaiter::tick_sockets(uint events) {
    bool found = false;
    for(auto fd = _files.begin(); fd != _files.end(); ++fd) {
        auto file = Activity::own().files()->get(*fd);
        if(file && file->check_events(events))
            found = true;
    }
    return found;
}

}
