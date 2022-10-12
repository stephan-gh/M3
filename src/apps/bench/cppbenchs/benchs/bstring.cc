/*
 * Copyright (C) 2020-2021 Nils Asmussen, Barkhausen Institut
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
#include <base/time/Profile.h>

#include <m3/Test.h>

#include "../cppbenchs.h"

using namespace m3;

static const size_t SIZE = 64 * 1024;

NOINLINE static void memcpy() {
    std::unique_ptr<word_t[]> src(new word_t[SIZE / sizeof(word_t)]);
    std::unique_ptr<word_t[]> dst(new word_t[SIZE / sizeof(word_t)]);

    Profile pr(5, 2);

    {
        auto name = OStringStream();
        format_to(name, "memcpy aligned {} KiB"_cf, SIZE / 1024);
        WVPERF(name.str(), pr.run<CycleInstant>([&src, &dst] {
            memcpy(dst.get(), src.get(), SIZE);
        }));
    }

    {
        auto name = OStringStream();
        format_to(name, "memcpy unaligned {} KiB"_cf, SIZE / 1024);
        WVPERF(name.str(), pr.run<CycleInstant>([&src, &dst] {
            memcpy(reinterpret_cast<char *>(dst.get()) + 1, src.get(), SIZE - 1);
        }));
    }
}

NOINLINE static void memset() {
    std::unique_ptr<word_t[]> dst(new word_t[SIZE / sizeof(word_t)]);

    Profile pr(5, 2);

    {
        auto name = OStringStream();
        format_to(name, "memset {} KiB"_cf, SIZE / 1024);
        WVPERF(name.str(), pr.run<CycleInstant>([&dst] {
            memset(dst.get(), 0, SIZE);
        }));
    }
}

NOINLINE static void memmove() {
    std::unique_ptr<char[]> buf(new char[SIZE * 2]);

    Profile pr(5, 2);

    {
        auto name = OStringStream();
        format_to(name, "memmove backwards {} KiB"_cf, SIZE / 1024);
        WVPERF(name.str(), pr.run<CycleInstant>([&buf] {
            memmove(buf.get(), buf.get() + SIZE, SIZE);
        }));
    }

    {
        auto name = OStringStream();
        format_to(name, "memmove overlapping unaligned {} KiB"_cf, SIZE / 1024);
        WVPERF(name.str(), pr.run<CycleInstant>([&buf] {
            memmove(buf.get() + 1, buf.get(), SIZE - 1);
        }));
    }

    {
        auto name = OStringStream();
        format_to(name, "memmove overlapping aligned {} KiB"_cf, SIZE / 1024);
        WVPERF(name.str(), pr.run<CycleInstant>([&buf] {
            memmove(buf.get() + sizeof(word_t), buf.get(), SIZE - sizeof(word_t));
        }));
    }

    {
        auto name = OStringStream();
        format_to(name, "memmove forward {} KiB"_cf, SIZE / 1024);
        WVPERF(name.str(), pr.run<CycleInstant>([&buf] {
            memmove(buf.get() + SIZE, buf.get(), SIZE);
        }));
    }
}

NOINLINE static void memcmp() {
    std::unique_ptr<char[]> b1(new char[SIZE]);
    std::unique_ptr<char[]> b2(new char[SIZE]);

    Profile pr(5, 2);

    memset(b1.get(), 0xAA, SIZE);
    memset(b2.get(), 0xAA, SIZE);

    {
        auto name = OStringStream();
        format_to(name, "memcmp succ {} KiB"_cf, SIZE / 1024);
        WVPERF(name.str(), pr.run<CycleInstant>([&b1, &b2] {
            WVASSERTEQ(memcmp(b1.get(), b2.get(), SIZE), 0);
        }));
    }

    memset(b2.get(), 0xBB, SIZE);

    {
        auto name = OStringStream();
        format_to(name, "memcmp fail {} KiB"_cf, SIZE / 1024);
        WVPERF(name.str(), pr.run<CycleInstant>([&b1, &b2] {
            WVASSERT(memcmp(b1.get(), b2.get(), SIZE) < 0);
        }));
    }
}

void bstring() {
    RUN_BENCH(memcpy);
    RUN_BENCH(memset);
    RUN_BENCH(memmove);
    RUN_BENCH(memcmp);
}
