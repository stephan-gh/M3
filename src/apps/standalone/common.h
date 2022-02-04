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
#include <base/TCU.h>
#include <heap/heap.h>
#include <string.h>

#include "assert.h"
#include "tcuif.h"
#include "tiles.h"

template<size_t PAD>
struct UnalignedData {
    uint8_t _pad[PAD];
    uint64_t pre;
    uint64_t data[3];
    uint64_t post;
} PACKED ALIGNED(16);

#define RUN_SUITE(name)                                                          \
    m3::Serial::get() << "Running testsuite " << #name << " ...\n";              \
    name();                                                                      \
    m3::Serial::get() << "\n";

extern void test_msgs();
extern void test_mem();
extern void test_ext();
