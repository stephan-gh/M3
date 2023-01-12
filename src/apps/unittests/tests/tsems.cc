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

#include <m3/Test.h>
#include <m3/com/Semaphore.h>
#include <m3/stream/FStream.h>
#include <m3/tiles/ChildActivity.h>
#include <m3/vfs/VFS.h>

#include "../unittests.h"

using namespace m3;

static int get_counter(const char *filename) {
    char buffer[8] = {0};
    auto file = VFS::open(filename, FILE_R);
    file->read(buffer, sizeof(buffer));
    return IStringStream::read_from<int>(buffer);
}

static void set_counter(const char *filename, int value) {
    char buffer[8];
    OStringStream os(buffer, sizeof(buffer));
    format_to(os, "{}"_cf, value);

    auto file = VFS::open(filename, FILE_W | FILE_TRUNC | FILE_CREATE);
    file->write(os.str(), os.length());
}

static void taking_turns() {
    Semaphore sem0 = Semaphore::create(1);
    Semaphore sem1 = Semaphore::create(0);

    set_counter("/sem0", 0);
    set_counter("/sem1", 0);

    auto tile = Tile::get("compat|own");
    ChildActivity child(tile, "child");

    child.delegate_obj(sem0.sel());
    child.delegate_obj(sem1.sel());

    child.add_mount("/", "/");

    child.data_sink() << sem0.sel() << sem1.sel();

    child.run([] {
        capsel_t sem0_sel, sem1_sel;
        Activity::own().data_source() >> sem0_sel >> sem1_sel;

        Semaphore sem0 = Semaphore::bind(sem0_sel);
        Semaphore sem1 = Semaphore::bind(sem1_sel);
        for(int i = 0; i < 10; ++i) {
            sem0.down();
            WVASSERTEQ(get_counter("/sem0"), i);
            set_counter("/sem1", i);
            sem1.up();
        }
        return failed ? 1 : 0;
    });

    for(int i = 0; i < 10; ++i) {
        sem1.down();
        WVASSERTEQ(get_counter("/sem1"), i);
        set_counter("/sem0", i + 1);
        sem0.up();
    }

    WVASSERTEQ(child.wait(), 0);
}

void tsems() {
    RUN_TEST(taking_turns);
}
