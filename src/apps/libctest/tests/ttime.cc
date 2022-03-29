/*
 * Copyright (C) 2021 Nils Asmussen, Barkhausen Institut
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

#define _GNU_SOURCE
#include <sys/time.h>
#include <time.h>

#include "../libctest.h"

using namespace m3;

static void basics() {
    struct timeval t;
    t.tv_sec = 0xDEAD;
    t.tv_usec = 0xBEEF;
    WVASSERTEQ(gettimeofday(&t, nullptr), 0);
    WVASSERT(t.tv_sec != 0xDEAD);
    WVASSERT(t.tv_usec != 0xBEEF);

    struct timespec ts;
    ts.tv_sec = 0xDEAD;
    ts.tv_nsec = 0xBEEF;
    WVASSERTEQ(clock_gettime(CLOCK_MONOTONIC, &ts), 0);
    WVASSERT(ts.tv_sec != 0xDEAD);
    WVASSERT(ts.tv_nsec != 0xBEEF);
}

void ttime() {
    RUN_TEST(basics);
}
