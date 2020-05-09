#pragma once

#ifndef PACKED
#   define PACKED      __attribute__((packed))
#endif
#ifndef UNREACHED
#   define UNREACHED   __builtin_unreachable()
#endif

typedef unsigned char uint8_t;
typedef unsigned short uint16_t;
typedef unsigned int uint32_t;
typedef unsigned long long uint64_t;

typedef unsigned long epid_t;
typedef unsigned long peid_t;
typedef unsigned vpeid_t;
typedef unsigned long word_t;
typedef uint32_t label_t;
typedef uint16_t crd_t;
typedef uint64_t reg_t;
typedef uint64_t goff_t;

#if defined(__x86_64__)
#   include "x86_64/asm.h"
#elif defined(__arm__)
#   include "arm/asm.h"
#elif defined(__riscv__) || defined(__riscv)
#   include "riscv/asm.h"
#else
#   error "Unsupported ISA"
#endif

#if defined(__hw__)
#   include "hw/platform.h"
#elif defined(__gem5__)
#   include "gem5/platform.h"
#else
#   error "Unsupported platform"
#endif

inline void compiler_barrier() {
    asm volatile ("" : : : "memory");
}
