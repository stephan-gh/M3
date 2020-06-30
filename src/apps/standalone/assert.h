#pragma once

#include <base/stream/Serial.h>

#define ASSERT(a) ASSERT_EQ(a, true)
#define ASSERT_EQ(a, b) do {                                                                    \
        if((a) != (b)) {                                                                        \
            m3::Serial::get() << "assert in " << __FILE__ << ":" << __LINE__                    \
                              << " failed: received " << (a) << ", expected " << (b) << "\n";   \
            exit(1);                                                                            \
        }                                                                                       \
    } while(0)
