/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019 Nils Asmussen, Barkhausen Institut
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
#include <base/util/BitField.h>

#include <m3/Test.h>

#include "../unittests.h"

using namespace m3;

static void first_clear() {
    {
        BitField<16> bf;
        WVASSERTEQ(bf.first_clear(), 0u);

        bf.set(0);
        WVASSERTEQ(bf.first_clear(), 1u);

        bf.set(1);
        WVASSERTEQ(bf.first_clear(), 2u);

        bf.set(3);
        WVASSERTEQ(bf.first_clear(), 2u);

        for(uint i = 0; i < 16; ++i)
            bf.set(i);
        WVASSERTEQ(bf.first_clear(), 16u);
    }

    {
        BitField<65> bf;

        bf.set(33);
        WVASSERTEQ(bf.first_clear(), 0u);

        for(uint i = 0; i < 65; ++i)
            bf.set(i);
        WVASSERTEQ(bf.first_clear(), 65u);
    }

    {
        BitField<10> bf;
        for(uint i = 0; i < 10; ++i)
            bf.set(i);
        WVASSERTEQ(bf.first_clear(), 10u);

        bf.clear(9);
        WVASSERTEQ(bf.first_clear(), 9u);

        bf.clear(3);
        WVASSERTEQ(bf.first_clear(), 3u);

        bf.set(3);
        WVASSERTEQ(bf.first_clear(), 9u);

        bf.clear(6);
        bf.clear(7);
        WVASSERTEQ(bf.first_clear(), 6u);

        bf.set(6);
        WVASSERTEQ(bf.first_clear(), 7u);

        bf.set(9);
        WVASSERTEQ(bf.first_clear(), 7u);

        bf.set(7);
        WVASSERTEQ(bf.first_clear(), 10u);
    }
}

void tbitfield() {
    RUN_TEST(first_clear);
}
