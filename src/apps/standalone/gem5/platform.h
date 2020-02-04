#pragma once

#include <stdlib.h>

extern "C" int puts(const char *str);

#define OWN_MODID         0
#define MEM_MODID         1

#define STRINGIFY(x) #x
#define TOSTRING(x) STRINGIFY(x)

#define ASSERT(a) ASSERT_EQ(a, true)
#define ASSERT_EQ(a, b) do {            \
        if((a) != (b)) {                \
            puts("\e[1massert in ");    \
            puts(__FILE__);             \
            puts(":");                  \
            puts(TOSTRING(__LINE__));   \
            puts(" failed\e[0m\n");     \
            exit(1);                    \
        }                               \
    } while(0)

void init();
void deinit();
