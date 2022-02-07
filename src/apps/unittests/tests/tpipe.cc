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

#include <m3/pipe/DirectPipe.h>
#include <m3/vfs/FileRef.h>
#include <m3/Test.h>

#include "../unittests.h"

using namespace m3;

static char buffer[0x100];

static void reader_quit() {
    auto tile = Tile::get("clone|own");
    Activity writer(tile, "writer");
    MemGate mem = MemGate::create_global(0x1000, MemGate::RW);
    DirectPipe pipe(Activity::self(), writer, mem, 0x1000);

    writer.files()->set(STDIN_FD, Activity::self().files()->get(STDIN_FD));
    writer.files()->set(STDOUT_FD, Activity::self().files()->get(pipe.writer_fd()));

    writer.run([] {
        auto out = Activity::self().files()->get(STDOUT_FD);
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
    auto tile = Tile::get("clone|own");
    Activity reader(tile, "reader");

    MemGate mem = MemGate::create_global(64, MemGate::RW);
    DirectPipe pipe(reader, Activity::self(), mem, 64);

    reader.files()->set(STDIN_FD, Activity::self().files()->get(pipe.reader_fd()));
    reader.files()->set(STDOUT_FD, Activity::self().files()->get(STDOUT_FD));

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
    auto tile1 = Tile::get("clone|own");
    auto tile2 = Tile::get("clone|own");
    Activity reader(tile1, "reader");
    Activity writer(tile2, "writer");
    MemGate mem = MemGate::create_global(0x1000, MemGate::RW);
    DirectPipe pipe(reader, writer, mem, 0x1000);

    reader.files()->set(STDIN_FD, Activity::self().files()->get(pipe.reader_fd()));
    reader.files()->set(STDOUT_FD, Activity::self().files()->get(STDOUT_FD));

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

    writer.files()->set(STDIN_FD, Activity::self().files()->get(STDIN_FD));
    writer.files()->set(STDOUT_FD, Activity::self().files()->get(pipe.writer_fd()));

    writer.run([] {
        auto out = Activity::self().files()->get(STDOUT_FD);
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
