#pragma once

typedef unsigned long size_t;
typedef unsigned long uintptr_t;

inline void memory_barrier() {
    asm volatile ("fence");
}

inline uint64_t read8b(uintptr_t addr) {
    uint64_t res;
    asm volatile (
        "ld %0, (%1)"
        : "=r"(res)
        : "r"(addr)
    );
    return res;
}

inline void write8b(uintptr_t addr, uint64_t val) {
    asm volatile (
        "sd %0, (%1)"
        : : "r"(val), "r"(addr)
    );
}
