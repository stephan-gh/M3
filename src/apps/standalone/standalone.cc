/*
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

#include "common.h"

using namespace m3;

int main() {
    Serial::get() << "Starting TCU tests\n\n";

    RUN_SUITE(test_msgs);
    RUN_SUITE(test_mem);
    RUN_SUITE(test_ext);

    Serial::get() << "\x1B[1;32mAll tests successful!\x1B[0;m\n";
    // for the test infrastructure
    Serial::get() << "Shutting down\n";
    return 0;
}
