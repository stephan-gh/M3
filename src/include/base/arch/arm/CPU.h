/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2021 Nils Asmussen, Barkhausen Institut
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

#include <base/CPU.h>
#include <base/Common.h>

#define NEED_ALIGNED_MEMACC 0

namespace m3 {

inline uint64_t CPU::read8b(uintptr_t addr) {
    uint64_t res;
    asm volatile("ldrd %0, [%1]" : "=r"(res) : "r"(addr));
    return res;
}

inline void CPU::write8b(uintptr_t addr, uint64_t val) {
    asm volatile("strd %0, [%1]" : : "r"(val), "r"(addr));
}

ALWAYS_INLINE word_t CPU::base_pointer() {
    word_t val;
    asm volatile("mov %0, r11;" : "=r"(val));
    return val;
}

ALWAYS_INLINE word_t CPU::stack_pointer() {
    word_t val;
    asm volatile("mov %0, r13;" : "=r"(val));
    return val;
}

inline cycles_t CPU::elapsed_cycles() {
    // TODO for now we use our custom instruction
    return gem5_debug(0);
}

inline uintptr_t CPU::backtrace_step(uintptr_t bp, uintptr_t *func) {
    *func = reinterpret_cast<uintptr_t *>(bp)[0];
    return reinterpret_cast<uintptr_t *>(bp)[-1];
}

inline void CPU::compute(cycles_t cycles) {
    asm volatile(
        ".align 4;"
        "1: subs %0, %0, #1;"
        "bgt     1b;"
        // let the compiler know that we change the value of cycles
        // as it seems, inputs are not expected to change
        : "=r"(cycles)
        : "0"(cycles));
}

inline void CPU::memory_barrier() {
    asm volatile("dmb" : : : "memory");
}

inline cycles_t CPU::gem5_debug(uint64_t msg) {
    // TODO for now we use our custom instruction
    register uint32_t r0 asm("r0") = msg & 0xFFFF'FFFF;
    register uint32_t r1 asm("r1") = msg >> 32;
    asm volatile(".long 0xEE630110" : "+r"(r0), "+r"(r1));
    return static_cast<uint64_t>(r0) | (static_cast<uint64_t>(r1) << 32);
}

}
