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
     * Adds the given file descriptor to the set of files that this FileWaiter waits for. This
     * method assumes that the file descriptor has not been given to this waiter yet.
     *
     * @param fd the file descriptor to add
     * @param events the events you are interested in for this file (see File::Event)
     */
    void add(fd_t fd, uint events) {
        _files.push_back(std::make_pair(fd, events));
    }

    /**
     * Adds or sets the given events for the given file descriptor. If the file descriptor already
     * exists, the events are updated. Otherwise, a new entry is created.
     *
     * @param fd the file descriptor to set the events for
     * @param events the events you are interested in for this file (see File::Event)
     */
    void set(fd_t fd, uint events) {
        auto existing =
            std::find_if(_files.begin(), _files.end(), [fd](const std::pair<fd_t, uint> &f) {
                return f.first == fd;
            });
        if(existing != _files.end())
            existing->second = events;
        else
            add(fd, events);
    }

    /**
     * Removes the given file descriptor from the set of files that this FileWaiter waits for.
     *
     * @param fd the file descriptor to remove
     */
    void remove(fd_t fd) {
        _files.erase(std::remove_if(_files.begin(), _files.end(),
                                    [fd](const std::pair<fd_t, uint> &f) {
                                        return f.first == fd;
                                    }),
                     _files.end());
    }

    /**
     * Waits until any file has received any of the desired events.
     *
     * Note: this function uses Activity::sleep if tick_sockets returns false, which suspends the
     * core until the next TCU message arrives. Thus, calling this function can only be done if all
     * work is done.
     */
    void wait();

    /**
     * Waits until any file has received any of the desired events or the given timeout in
     * nanoseconds is reached.
     *
     * Note: this function uses Activity::sleep if tick_sockets returns false, which suspends the
     * core until the next TCU message arrives. Thus, calling this function can only be done if all
     * work is done.
     *
     * @param timeout the maximum time to wait
     */
    void wait_for(TimeDuration timeout);

private:
    bool tick_files();

    std::vector<std::pair<fd_t, uint>> _files;
};

}
