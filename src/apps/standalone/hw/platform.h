#pragma once

#include <stdlib.h>

//MODIDs
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

#define OWN_MODID         MODID_PM6
#define MEM_MODID         MODID_DRAM1

#define ASSERT(a) ASSERT_EQ(a, true)
#define ASSERT_EQ(a, b) do { \
        if((a) != (b)) { \
            ui32_ptr[0]=a; \
            ui32_ptr[1]=__LINE__; \
            exit(1); \
        } \
    } while(0)

extern volatile uint64_t *ui64_ptr;
extern volatile uint32_t *ui32_ptr;

void init();
void deinit();
