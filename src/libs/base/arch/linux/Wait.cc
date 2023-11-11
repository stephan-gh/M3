/*
 * Copyright (C) 2023 Nils Asmussen, Barkhausen Institut
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

#include <base/Init.h>
#include <base/arch/linux/Init.h>
#include <base/arch/linux/Wait.h>

#include <sys/epoll.h>

namespace m3lx {

struct LinuxWait {
    LinuxWait();

    int fd;
};

static INIT_PRIO_LXWAIT LinuxWait lxwait;

LinuxWait::LinuxWait() {
    // size argument is ignored since 2.6.8, but needs to be non-zero
    fd = epoll_create(1);
    assert(fd != -1);

    struct epoll_event ev;
    ev.data.fd = tcu_fd();
    ev.events = EPOLLIN;
    epoll_ctl(fd, EPOLL_CTL_ADD, tcu_fd(), &ev);
}

void wait_msg(m3::TimeDuration timeout) {
    int timeout_ms;
    if(timeout == m3::TimeDuration::MAX)
        timeout_ms = -1;
    else
        timeout_ms = timeout.as_millis();

    struct epoll_event ev;
    epoll_wait(lxwait.fd, &ev, 1, timeout_ms);
}

}
