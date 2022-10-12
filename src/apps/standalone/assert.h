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

#pragma once

#include <base/stream/Serial.h>

#define ASSERT(a) ASSERT_EQ(a, true)
#define ASSERT_EQ(a, b)                                                                  \
    do {                                                                                 \
        auto __a = (a);                                                                  \
        decltype(__a) __b = (b);                                                         \
        if(__a != __b) {                                                                 \
            logln("! {}:{} \"{}\" == \"{}\" FAILED\n"_cf, __FILE__, __LINE__, __a, __b); \
            exit(1);                                                                     \
        }                                                                                \
    }                                                                                    \
    while(0)
