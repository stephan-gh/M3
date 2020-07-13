#pragma once

#include <base/stream/Serial.h>

#define ASSERT(a) ASSERT_EQ(a, true)
#define ASSERT_EQ(a, b) do {                                                         \
        auto __a = (a);                                                              \
        decltype(__a) __b = (b);                                                     \
        if(__a != __b) {                                                             \
            m3::Serial::get() << "! " << __FILE__ << ":" << __LINE__                 \
                              << " \"" << __a << "\" == \"" << __b << "\" FAILED\n"; \
            exit(1);                                                                 \
        }                                                                            \
    } while(0)
