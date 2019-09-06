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

#include <base/stream/IStringStream.h>
#include <base/stream/OStringStream.h>

#include <m3/pipe/DirectPipe.h>
#include <m3/vfs/FileRef.h>
#include <m3/Test.h>

#include "../unittests.h"

using namespace m3;

static char buffer[0x100];

static void reader_quit() {
    VPE writer("writer");
    MemGate mem = MemGate::create_global(0x1000, MemGate::RW);
    DirectPipe pipe(VPE::self(), writer, mem, 0x1000);

    writer.fds()->set(STDIN_FD, VPE::self().fds()->get(STDIN_FD));
    writer.fds()->set(STDOUT_FD, VPE::self().fds()->get(pipe.writer_fd()));
    writer.obtain_fds();

    writer.run([] {
        auto out = VPE::self().fds()->get(STDOUT_FD);
        while(1) {
            OStringStream os(buffer, sizeof(buffer));
            os << "Hello World!\n";
            if(out->write(buffer, os.length()) == 0)
                break;
        }
        return 0;
    });

    pipe.close_writer();

    {
        FStream in(pipe.reader_fd());
        size_t count = in.getline(buffer, sizeof(buffer));
        WVASSERTEQ(count, 12u);
        WVASSERTEQ(buffer, StringRef("Hello World!"));
        count = in.getline(buffer, sizeof(buffer));
        WVASSERTEQ(count, 12u);
        WVASSERTEQ(buffer, StringRef("Hello World!"));
    }

    pipe.close_reader();

    WVASSERTEQ(writer.wait(), 0);
}

static void writer_quit() {
    VPE reader("reader");

    MemGate mem = MemGate::create_global(64, MemGate::RW);
    DirectPipe pipe(reader, VPE::self(), mem, 64);

    reader.fds()->set(STDIN_FD, VPE::self().fds()->get(pipe.reader_fd()));
    reader.fds()->set(STDOUT_FD, VPE::self().fds()->get(STDOUT_FD));
    reader.obtain_fds();

    reader.run([] {
        size_t count = cin.getline(buffer, sizeof(buffer));
        WVASSERTEQ(count, 12u);
        WVASSERTEQ(buffer, StringRef("Hello World!"));
        count = cin.getline(buffer, sizeof(buffer));
        WVASSERTEQ(count, 12u);
        WVASSERTEQ(buffer, StringRef("Hello World!"));
        count = cin.getline(buffer, sizeof(buffer));
        WVASSERTEQ(count, 0u);
        return failed ? 1 : 0;
    });

    pipe.close_reader();

    {
        FStream f(pipe.writer_fd(), FILE_W);
        for(int i = 0; i < 2; ++i)
            f << "Hello World!\n";
    }

    pipe.close_writer();

    WVASSERTEQ(reader.wait(), 0);
}

static void child_to_child() {
    VPE reader("reader");
    VPE writer("writer");
    MemGate mem = MemGate::create_global(0x1000, MemGate::RW);
    DirectPipe pipe(reader, writer, mem, 0x1000);

    reader.fds()->set(STDIN_FD, VPE::self().fds()->get(pipe.reader_fd()));
    reader.fds()->set(STDOUT_FD, VPE::self().fds()->get(STDOUT_FD));
    reader.obtain_fds();

    reader.run([] {
        for(int i = 0; i < 10; ++i) {
            size_t count = cin.getline(buffer, sizeof(buffer));
            WVASSERTEQ(count, 12u);
            WVASSERTEQ(buffer, StringRef("Hello World!"));
        }
        size_t count = cin.getline(buffer, sizeof(buffer));
        WVASSERTEQ(count, 0u);
        return failed ? 1 : 0;
    });

    writer.fds()->set(STDIN_FD, VPE::self().fds()->get(STDIN_FD));
    writer.fds()->set(STDOUT_FD, VPE::self().fds()->get(pipe.writer_fd()));
    writer.obtain_fds();

    writer.run([] {
        auto out = VPE::self().fds()->get(STDOUT_FD);
        for(int i = 0; i < 10; ++i) {
            OStringStream os(buffer, sizeof(buffer));
            os << "Hello World!\n";
            out->write(buffer, os.length());
        }
        return 0;
    });

    pipe.close_writer();
    pipe.close_reader();

    WVASSERTEQ(reader.wait(), 0);
    WVASSERTEQ(writer.wait(), 0);
}

void tpipe() {
    RUN_TEST(reader_quit);
    RUN_TEST(writer_quit);
    RUN_TEST(child_to_child);
}
