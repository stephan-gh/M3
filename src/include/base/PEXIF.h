/*
 * Copyright (C) 2016, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
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

#include <base/Common.h>

namespace m3 {

typedef uint32_t irq_t;

static constexpr irq_t INVALID_IRQ = static_cast<irq_t>(-1);

enum Operation : word_t {
    WAIT,
    EXIT,
    YIELD,
    MAP,
    REG_IRQ,
    TRANSL_FAULT,
    FLUSH_INV,
    NOOP,
};

}

#if defined(__x86_64__)
#   include "arch/x86_64/PEXABI.h"
#elif defined(__arm__)
#   include "arch/arm/PEXABI.h"
#elif defined(__riscv)
#   include "arch/riscv/PEXABI.h"
#else
#   error "Unsupported ISA"
#endif

namespace m3 {

struct PEXIF {
    static void wait(epid_t ep, irq_t irq, uint64_t nanos = 0xFFFFFFFFFFFFFFFF) {
        PEXABI::call3(Operation::WAIT, ep, irq, nanos);
    }

    static void exit(int code) {
        PEXABI::call1(Operation::EXIT, static_cast<word_t>(code));
    }

    static void map(uintptr_t virt, goff_t phys, size_t pages, uint perm) {
        PEXABI::call4(Operation::MAP, virt, phys, pages, perm);
    }

    static void reg_irq(irq_t irq) {
        PEXABI::call1(Operation::REG_IRQ, irq);
    }

    static void flush_invalidate() {
        PEXABI::call2(Operation::FLUSH_INV, 0, 0);
    }
};

}
