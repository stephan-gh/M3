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

#include <unistd.h>

#include "../libctest.h"

using namespace m3;

static void basics() {
    WVASSERTEQ(getpid(), Activity::own().id() + 1);
    WVASSERTEQ(getuid(), 0U);
    WVASSERTEQ(geteuid(), 0U);
    WVASSERTEQ(getgid(), 0U);
    WVASSERTEQ(getegid(), 0U);
}

void tprocess() {
    RUN_TEST(basics);
}
