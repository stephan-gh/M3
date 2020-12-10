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

enum class PE {
    PE0,
    PE1,
    PE2,
    PE3,
    PE4,
    PE5,
    PE6,
    PE7,
    MEM,
};

static uint PE_IDS[][9] = {
    // platform = gem5
    { 0, 1, 2, 3, 4, 5, 6, 7, 8 },
    // platform = hw
    { MODID_PM0, MODID_PM1, MODID_PM2, MODID_PM3,
      MODID_PM4, MODID_PM5, MODID_PM6, MODID_PM7,
      MODID_DRAM2 },
};

static inline uint pe_id(PE pe) {
    return PE_IDS[m3::env()->platform][static_cast<size_t>(pe)];
}
