/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
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

#include <base/Common.h>
#include <base/TCU.h>

#include <string.h>

#include "assert.h"
#include "tcuif.h"
#include "tiles.h"

template<size_t PAD>
struct UnalignedData {
    uint8_t _pad[PAD];
    uint8_t pre;
    uint8_t data[16];
    uint8_t post;
} PACKED ALIGNED(16);

#define RUN_SUITE(name)                          \
    m3::logln("Running testsuite {}"_cf, #name); \
    name();                                      \
    m3::logln();

extern void test_msgs();
extern void test_mem();
extern void test_ext();
