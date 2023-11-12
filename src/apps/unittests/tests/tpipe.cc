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

#include <m3/Test.h>
#include <m3/pipe/DirectPipe.h>
#include <m3/tiles/ChildActivity.h>
#include <m3/vfs/FileRef.h>

#include "../unittests.h"

using namespace m3;

static char buffer[0x100];

static void reader_quit() {
    auto tile = Tile::get("compat|own");
    ChildActivity writer(tile, "writer");
    MemCap mem = MemCap::create_global(0x1000, MemCap::RW);
    DirectPipe pipe(Activity::own(), writer, mem, 0x1000);

    writer.add_file(STDIN_FD, STDIN_FD);
    writer.add_file(STDOUT_FD, pipe.writer_fd());

    writer.run([] {
        auto out = Activity::own().files()->get(STDOUT_FD);
        while(1) {
            OStringStream os(buffer, sizeof(buffer));
            format_to(os, "Hello World!\n"_cf);
            if(out->write(buffer, os.length()).unwrap() == 0)
                break;
        }
        return 0;
    });

    pipe.close_writer();

    {
        FStream in(pipe.reader_fd());
        size_t count = in.getline(buffer, sizeof(buffer));
        WVASSERTEQ(count, 12u);
        WVASSERTSTREQ(buffer, "Hello World!");
        count = in.getline(buffer, sizeof(buffer));
        WVASSERTEQ(count, 12u);
        WVASSERTSTREQ(buffer, "Hello World!");
    }

    pipe.close_reader();

    WVASSERTEQ(writer.wait(), 0);
}

static void writer_quit() {
    auto tile = Tile::get("compat|own");
    ChildActivity reader(tile, "reader");

    MemCap mem = MemCap::create_global(64, MemCap::RW);
    DirectPipe pipe(reader, Activity::own(), mem, 64);

    reader.add_file(STDIN_FD, pipe.reader_fd());
    reader.add_file(STDOUT_FD, STDOUT_FD);

    reader.run([] {
        size_t count = cin.getline(buffer, sizeof(buffer));
        WVASSERTEQ(count, 12u);
        WVASSERTSTREQ(buffer, "Hello World!");
        count = cin.getline(buffer, sizeof(buffer));
        WVASSERTEQ(count, 12u);
        WVASSERTSTREQ(buffer, "Hello World!");
        count = cin.getline(buffer, sizeof(buffer));
        WVASSERTEQ(count, 0u);
        return failed ? 1 : 0;
    });

    pipe.close_reader();

    {
        FStream f(pipe.writer_fd(), FILE_W);
        for(int i = 0; i < 2; ++i)
            println_to(f, "Hello World!"_cf);
    }

    pipe.close_writer();

    WVASSERTEQ(reader.wait(), 0);
}

static void child_to_child() {
    auto tile1 = Tile::get("compat|own");
    auto tile2 = Tile::get("compat|own");
    ChildActivity reader(tile1, "reader");
    ChildActivity writer(tile2, "writer");
    MemCap mem = MemCap::create_global(0x1000, MemCap::RW);
    DirectPipe pipe(reader, writer, mem, 0x1000);

    reader.add_file(STDIN_FD, pipe.reader_fd());
    reader.add_file(STDOUT_FD, STDOUT_FD);

    reader.run([] {
        for(int i = 0; i < 10; ++i) {
            size_t count = cin.getline(buffer, sizeof(buffer));
            WVASSERTEQ(count, 12u);
            WVASSERTSTREQ(buffer, "Hello World!");
        }
        size_t count = cin.getline(buffer, sizeof(buffer));
        WVASSERTEQ(count, 0u);
        return failed ? 1 : 0;
    });

    writer.add_file(STDIN_FD, STDIN_FD);
    writer.add_file(STDOUT_FD, pipe.writer_fd());

    writer.run([] {
        auto out = Activity::own().files()->get(STDOUT_FD);
        for(int i = 0; i < 10; ++i) {
            OStringStream os(buffer, sizeof(buffer));
            format_to(os, "Hello World!\n"_cf);
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
