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

#pragma once

#include <base/Common.h>
#include <base/time/Duration.h>

#include <algorithm>
#include <vector>

namespace m3 {

class FileWaiter {
public:
    explicit FileWaiter() : _files() {
    }

    /**
     * Adds the given file descriptor to the set of files that this FileWaiter waits for.
     *
     * @param fd the file descriptor to add
     */
    void add(fd_t fd) {
        _files.push_back(fd);
    }

    /**
     * Removes the given file descriptor from the set of files that this FileWaiter waits for.
     *
     * @param fd the file descriptor to remove
     */
    void remove(fd_t fd) {
        _files.erase(std::remove(_files.begin(), _files.end(), fd));
    }

    /**
     * Waits until any file has received any of the given events.
     *
     * Note: this function uses Activity::sleep if tick_sockets returns false, which suspends the core
     * until the next TCU message arrives. Thus, calling this function can only be done if all work
     * is done.
     *
     * @param events the events to wait for (see File::Event)
     */
    void wait(uint events);

    /**
     * Waits until any file has received any of the given events or the given timeout in nanoseconds
     * is reached.
     *
     * Note: this function uses Activity::sleep if tick_sockets returns false, which suspends the core
     * until the next TCU message arrives. Thus, calling this function can only be done if all work
     * is done.
     *
     * @param timeout the maximum time to wait
     * @param events the events to wait for
     */
    void wait_for(TimeDuration timeout, uint events);

private:
    bool tick_sockets(uint events);

    std::vector<fd_t> _files;
};

}
