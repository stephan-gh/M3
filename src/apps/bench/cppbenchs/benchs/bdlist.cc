/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2021 Nils Asmussen, Barkhausen Institut
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
#include <base/Panic.h>
#include <base/col/DList.h>
#include <base/time/Profile.h>

#include <m3/Test.h>

#include "../cppbenchs.h"

using namespace m3;

struct MyDItem : public DListItem {
    explicit MyDItem(uint32_t _val) : val(_val) {
    }

    uint32_t val;
};

NOINLINE static void append() {
    struct DListAppendRunner : public Runner {
        void run() override {
            for(uint32_t i = 0; i < 10; ++i) {
                list.append(new MyDItem(i));
            }
        }
        void post() override {
            for(auto it = list.begin(); it != list.end();) {
                auto old = it++;
                delete &*old;
            }
            list.clear();
        }

        DList<MyDItem> list;
    };

    Profile pr(30, 100);
    DListAppendRunner runner;
    WVPERF("Appending 10-elements", pr.runner<CycleInstant>(runner));
}

NOINLINE static void clear() {
    struct DListClearRunner : public Runner {
        void pre() override {
            for(uint32_t i = 0; i < 10; ++i) {
                list.append(new MyDItem(i));
            }
        }
        void run() override {
            for(auto it = list.begin(); it != list.end();) {
                auto old = it++;
                delete &*old;
            }
            list.clear();
        }

        DList<MyDItem> list;
    };

    Profile pr(30, 100);
    DListClearRunner runner;
    WVPERF("Removing 10-elements", pr.runner<CycleInstant>(runner));
}

void bdlist() {
    RUN_BENCH(append);
    RUN_BENCH(clear);
}
