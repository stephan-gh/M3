/*
 * Copyright (C) 2021-2022 Nils Asmussen, Barkhausen Institut
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
#include <base/stream/Serial.h>
#include <base/time/Instant.h>
#include <base/util/Util.h>

#include "../assert.h"
#include "../tcuif.h"
#include "../tiles.h"

using namespace m3;

static constexpr epid_t MEP = TCU::FIRST_USER_EP;

static ALIGNED(8) uint8_t buf1[1024];
static ALIGNED(8) uint8_t buf2[1024];
static ALIGNED(8) uint8_t buf3[1024];
static ALIGNED(8) uint8_t zeros[1024];

int main() {
    TileId own_tile = TileId::from_raw(env()->tile_id);
    size_t own_idx = tile_idx(own_tile).unwrap();
    TileId partner_tile = TILE_IDS[(own_idx + 1) % 8];

    logln("Hello from {} (partner {})!"_cf, own_tile, partner_tile);

    kernel::TCU::config_mem(MEP, partner_tile, reinterpret_cast<uintptr_t>(buf1), sizeof(buf1),
                            TCU::R | TCU::W);

    for(size_t i = 0; i < ARRAY_SIZE(buf2); ++i)
        buf2[i] = own_tile.chip() + i;

    for(size_t off = 0; off < 16; ++off) {
        for(size_t sz = 0; sz < 16; ++sz) {
            logln("read-write off={}, sz={}"_cf, off, sz);
            for(int run = 0; run < 100; ++run) {
                size_t count = sz ? sz : (sizeof(buf2) - off);
                ASSERT_EQ(kernel::TCU::write(MEP, buf2 + off, count, 0), Errors::SUCCESS);
                ASSERT_EQ(kernel::TCU::read(MEP, buf3 + off, count, 0), Errors::SUCCESS);

                for(size_t i = 0; i < count; ++i)
                    ASSERT_EQ(buf2[off + i], buf3[off + i]);

                ASSERT_EQ(kernel::TCU::write(MEP, zeros, sizeof(zeros), 0), Errors::SUCCESS);
            }
        }
    }

    logln("\x1B[1;32mAll tests successful!\x1B[0;m"_cf);

    // give the other tiles some time
    auto end = TimeInstant::now() + TimeDuration::from_millis(10);
    while(TimeInstant::now() < end)
        ;

    // for the test infrastructure
    logln("Shutting down"_cf);
    return 0;
}
