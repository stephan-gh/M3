/*
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

#include <base/stream/IStringStream.h>
#include <base/stream/OStringStream.h>

#include <m3/pipe/IndirectPipe.h>
#include <m3/vfs/FileRef.h>
#include <m3/Test.h>

#include "../unittests.h"

using namespace m3;

static constexpr size_t PIPE_SIZE = 16;
static constexpr size_t DATA_SIZE = PIPE_SIZE / 4;

static void pipes() {
    Pipes pipes("pipes");
    MemGate mem = MemGate::create_global(PIPE_SIZE, MemGate::RW);
    IndirectPipe pipe(pipes, mem, PIPE_SIZE);

    auto fin = Activity::self().files()->get(pipe.reader_fd());
    auto fout = Activity::self().files()->get(pipe.writer_fd());
    fin->set_blocking(false);
    fout->set_blocking(false);

    char send_buf[DATA_SIZE] = {'t', 'e', 's', 't'};
    char recv_buf[DATA_SIZE];

    size_t count = 0;
    while(count < 100) {
        int progress = 0;

        ssize_t read = fin->read(recv_buf, sizeof(recv_buf));
        if(read != -1) {
            // this is actually not guaranteed, but depends on the implementation of the pipe
            // server. however, we want to ensure that the read data is correct, which is difficult
            // otherwise.
            WVASSERTEQ(read, static_cast<ssize_t>(sizeof(send_buf)));
            WVASSERT(strncmp(recv_buf, send_buf, sizeof(send_buf)) == 0);
            progress++;
            count += static_cast<size_t>(read);
        }

        ssize_t written = fout->write(send_buf, sizeof(send_buf));
        if(written != -1) {
            // see above
            WVASSERTEQ(written, static_cast<ssize_t>(sizeof(send_buf)));
            progress++;
        }

        if(count < 100 && progress == 0)
            Activity::sleep();
    }

    pipe.close_reader();
    pipe.close_writer();
}

void tnonblock() {
    RUN_TEST(pipes);
}
