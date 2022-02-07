/*
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

#pragma once

#include <base/Common.h>
#include <base/Env.h>

#define MODID_TILE0     0x04
#define MODID_TILE1     0x05
#define MODID_TILE2     0x06
#define MODID_TILE3     0x24
#define MODID_TILE4     0x25
#define MODID_TILE5     0x26
#define MODID_TILE6     0x00
#define MODID_TILE7     0x01
#define MODID_TILE8     0x02
#define MODID_TILE9     0x20
#define MODID_TILE10    0x21
#define MODID_TILE11    0x22

#define MODID_PM0       MODID_TILE2
#define MODID_PM1       MODID_TILE4
#define MODID_PM2       MODID_TILE5
#define MODID_PM3       MODID_TILE6
#define MODID_PM4       MODID_TILE7
#define MODID_PM5       MODID_TILE8
#define MODID_PM6       MODID_TILE9
#define MODID_PM7       MODID_TILE10

#define MODID_UART      MODID_TILE0
#define MODID_ETH       MODID_TILE1
#define MODID_DRAM1     MODID_TILE3
#define MODID_DRAM2     MODID_TILE11

enum class Tile {
    T0,
    T1,
    T2,
    T3,
    T4,
    T5,
    T6,
    T7,
    MEM,
};

static uint TILE_IDS[][9] = {
    // platform = gem5
    { 0, 1, 2, 3, 4, 5, 6, 7, 9 },
    // platform = hw
    { MODID_PM0, MODID_PM1, MODID_PM2, MODID_PM3,
      MODID_PM4, MODID_PM5, MODID_PM6, MODID_PM7,
      MODID_DRAM2 },
};

static inline uint tile_id(Tile tile) {
    return TILE_IDS[m3::env()->platform][static_cast<size_t>(tile)];
}
