#pragma once

#include <base/Common.h>
#include <base/Env.h>

// MODIDs
#define MODID_PM0         0x04
#define MODID_PM1         0x05
#define MODID_PM2         0x24
#define MODID_PM3         0x25
#define MODID_PM4         0x20
#define MODID_PM5         0x21
#define MODID_PM6         0x00
#define MODID_PM7         0x01

#define MODID_UART        MODID_PM0
#define MODID_ETH         MODID_PM1
#define MODID_DRAM1       MODID_PM2
#define MODID_DRAM2       MODID_PM4

#define MODID_ROUTER0     0x07
#define MODID_ROUTER1     0x27
#define MODID_ROUTER2     0x23
#define MODID_ROUTER3     0x03

enum class PE {
    PE0,
    PE1,
    PE2,
    PE3,
    MEM,
};

static uint PE_IDS[][5] = {
    // platform = gem5
    { 0, 1, 2, 3, 4 },
    // platform = hw
    { MODID_PM6, MODID_PM7, MODID_PM3, MODID_PM5, MODID_DRAM1 },
};

static inline uint pe_id(PE pe) {
    return PE_IDS[m3::env()->platform][static_cast<size_t>(pe)];
}
