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

#include <base/Common.h>

#include <m3/Test.h>
#include <m3/pipe/IndirectPipe.h>

#include <unistd.h>

#include "../libctest.h"
#include "sys/epoll.h"

using namespace m3;

static constexpr size_t PIPE_SIZE = 16;
static constexpr size_t DATA_SIZE = PIPE_SIZE / 4;

void tepoll() {
    Pipes pipes("pipes");
    MemCap mem = MemCap::create_global(PIPE_SIZE, MemCap::RW);
    IndirectPipe pipe(pipes, mem, PIPE_SIZE);

    pipe.reader().set_blocking(false);
    pipe.writer().set_blocking(false);

    char send_buf[DATA_SIZE] = {'t', 'e', 's', 't'};
    char recv_buf[DATA_SIZE];

    fd_t infd = pipe.reader().fd();
    fd_t outfd = pipe.writer().fd();

    int epfd = epoll_create(2);
    WVASSERT(epfd != -1);

    struct epoll_event events[2];
    events[0].events = EPOLLIN;
    events[0].data.fd = infd;
    WVASSERT(epoll_ctl(epfd, EPOLL_CTL_ADD, infd, &events[0]) != -1);
    events[1].events = EPOLLOUT;
    events[1].data.fd = outfd;
    WVASSERT(epoll_ctl(epfd, EPOLL_CTL_ADD, outfd, &events[1]) != -1);

    size_t count = 0;
    while(count < 100) {
        int ready = epoll_pwait(epfd, events, ARRAY_SIZE(events), -1, nullptr);
        for(int i = 0; i < ready; ++i) {
            ssize_t res;
            if(events[i].data.fd == infd) {
                WVASSERTEQ(events[i].events, static_cast<uint>(EPOLLIN));
                if((res = read(infd, recv_buf, sizeof(recv_buf))) > 0) {
                    // this is actually not guaranteed, but depends on the implementation of the
                    // pipe server. however, we want to ensure that the read data is correct, which
                    // is difficult otherwise.
                    WVASSERTEQ(static_cast<size_t>(res), sizeof(send_buf));
                    WVASSERT(strncmp(recv_buf, send_buf, sizeof(send_buf)) == 0);
                    count += static_cast<size_t>(res);
                }
            }
            else if(events[i].data.fd == outfd) {
                WVASSERTEQ(events[i].events, static_cast<uint>(EPOLLOUT));
                if((res = write(outfd, send_buf, sizeof(send_buf))) > 0) {
                    // see above
                    WVASSERTEQ(static_cast<size_t>(res), sizeof(send_buf));
                }
            }
        }
    }

    close(epfd);

    pipe.close_reader();
    pipe.close_writer();
}
