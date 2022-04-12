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

void FileWaiter::wait() {
    while(true) {
        if(tick_files())
            break;

        Activity::sleep();
    }
}

void FileWaiter::wait_for(TimeDuration timeout) {
    auto end = TimeInstant::now() + timeout;
    auto now = TimeInstant::now();
    while(now < end) {
        if(tick_files())
            break;

        Activity::sleep_for(end.duration_since(now));
        now = TimeInstant::now();
    }
}

bool FileWaiter::tick_files() {
    bool found = false;
    for(auto entry = _files.begin(); entry != _files.end(); ++entry) {
        auto file = Activity::own().files()->get(entry->first);
        if(file && file->check_events(entry->second))
            found = true;
    }
    return found;
}

}
