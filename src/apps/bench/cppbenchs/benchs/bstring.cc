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

#include <base/Common.h>
#include <base/util/Profile.h>
#include <base/Panic.h>

#include <m3/Test.h>

#include "../cppbenchs.h"

using namespace m3;

static const size_t SIZE = 64 * 1024;

NOINLINE static void memcpy() {
    std::unique_ptr<word_t[]> src(new word_t[SIZE / sizeof(word_t)]);
    std::unique_ptr<word_t[]> dst(new word_t[SIZE / sizeof(word_t)]);

    Profile pr(5, 2);

    WVPERF("memcpy aligned " << (SIZE / 1024) << " KiB", pr.run_with_id([&src, &dst] {
        memcpy(dst.get(), src.get(), SIZE);
    }, 0xA0));
    WVPERF("memcpy unaligned " << (SIZE / 1024) << " KiB", pr.run_with_id([&src, &dst] {
        memcpy(reinterpret_cast<char*>(dst.get()) + 1, src.get(), SIZE - 1);
    }, 0xA1));
}

NOINLINE static void memset() {
    std::unique_ptr<word_t[]> dst(new word_t[SIZE / sizeof(word_t)]);

    Profile pr(5, 2);

    WVPERF("memset " << (SIZE / 1024) << " KiB", pr.run_with_id([&dst] {
        memset(dst.get(), 0, SIZE);
    }, 0xA2));
}

NOINLINE static void memmove() {
    std::unique_ptr<char[]> buf(new char[SIZE * 2]);

    Profile pr(5, 2);

    WVPERF("memmove backwards " << (SIZE / 1024) << " KiB", pr.run_with_id([&buf] {
        memmove(buf.get(), buf.get() + SIZE, SIZE);
    }, 0xA3));
    WVPERF("memmove overlapping unaligned " << (SIZE / 1024) << " KiB", pr.run_with_id([&buf] {
        memmove(buf.get() + 1, buf.get(), SIZE - 1);
    }, 0xA3));
    WVPERF("memmove overlapping aligned " << (SIZE / 1024) << " KiB", pr.run_with_id([&buf] {
        memmove(buf.get() + sizeof(word_t), buf.get(), SIZE - sizeof(word_t));
    }, 0xA3));
    WVPERF("memmove forward " << (SIZE / 1024) << " KiB", pr.run_with_id([&buf] {
        memmove(buf.get() + SIZE, buf.get(), SIZE);
    }, 0xA4));
}

NOINLINE static void memcmp() {
    std::unique_ptr<char[]> b1(new char[SIZE]);
    std::unique_ptr<char[]> b2(new char[SIZE]);

    Profile pr(5, 2);

    memset(b1.get(), 0xAA, SIZE);
    memset(b2.get(), 0xAA, SIZE);

    WVPERF("memcmp succ " << (SIZE / 1024) << " KiB", pr.run_with_id([&b1, &b2] {
        WVASSERTEQ(memcmp(b1.get(), b2.get(), SIZE), 0);
    }, 0xA5));

    memset(b2.get(), 0xBB, SIZE);

    WVPERF("memcmp fail " << (SIZE / 1024) << " KiB", pr.run_with_id([&b1, &b2] {
        WVASSERT(memcmp(b1.get(), b2.get(), SIZE) < 0);
    }, 0xA6));
}

void bstring() {
    RUN_BENCH(memcpy);
    RUN_BENCH(memset);
    RUN_BENCH(memmove);
    RUN_BENCH(memcmp);
}