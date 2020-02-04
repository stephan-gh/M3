#pragma once

typedef unsigned int size_t;
typedef unsigned int uintptr_t;

inline void memory_barrier() {
    asm volatile ("dmb" : : : "memory");
}

inline uint64_t read8b(uintptr_t addr) {
    uint64_t res;
    asm volatile (
        "ldrd %0, [%1]"
        : "=r"(res)
        : "r"(addr)
    );
    return res;
}

inline void write8b(uintptr_t addr, uint64_t val) {
    asm volatile (
        "strd %0, [%1]"
        : : "r"(val), "r"(addr)
    );
}
