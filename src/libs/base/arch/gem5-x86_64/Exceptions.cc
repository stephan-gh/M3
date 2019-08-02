/*
 * Copyright (C) 2016-2017, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * This file is part of M3 (Microkernel for Minimalist Manycores).
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

#include <base/Common.h>
#include <base/stream/Serial.h>
#include <base/Backtrace.h>
#include <base/Exceptions.h>
#include <base/Env.h>

namespace m3 {

static word_t getCR2() {
    word_t res;
    asm volatile ("mov %%cr2, %0" : "=r"(res));
    return res;
}

static const char *exNames[] = {
    /* 0x00 */ "Divide by zero",
    /* 0x01 */ "Single step",
    /* 0x02 */ "Non maskable",
    /* 0x03 */ "Breakpoint",
    /* 0x04 */ "Overflow",
    /* 0x05 */ "Bounds check",
    /* 0x06 */ "Invalid opcode",
    /* 0x07 */ "Co-proc. n/a",
    /* 0x08 */ "Double fault",
    /* 0x09 */ "Co-proc seg. overrun",
    /* 0x0A */ "Invalid TSS",
    /* 0x0B */ "Segment not present",
    /* 0x0C */ "Stack exception",
    /* 0x0D */ "Gen. prot. fault",
    /* 0x0E */ "Page fault",
    /* 0x0F */ "<unknown>",
    /* 0x10 */ "Co-processor error",
};

OStream &operator<<(OStream &os, const Exceptions::State &state) {
    os << "Interruption @ " << fmt(state.rip, "p");
    if(state.intrptNo == 0xe)
        os << " for address " << fmt(getCR2(), "p");
    os << "\n  irq: ";
    if(state.intrptNo < ARRAY_SIZE(exNames))
        os << exNames[state.intrptNo];
    else if(state.intrptNo == 64)
        os << "DTU (" << state.intrptNo << ")";
    else
        os << "<unknown> (" << state.intrptNo << ")";
    os << "\n";

    Backtrace::print(os);

    os << "  err: " << state.errorCode << "\n";
    os << "  rax: " << fmt(state.rax,    "#0x", 16) << "\n";
    os << "  rbx: " << fmt(state.rbx,    "#0x", 16) << "\n";
    os << "  rcx: " << fmt(state.rcx,    "#0x", 16) << "\n";
    os << "  rdx: " << fmt(state.rdx,    "#0x", 16) << "\n";
    os << "  rsi: " << fmt(state.rsi,    "#0x", 16) << "\n";
    os << "  rdi: " << fmt(state.rdi,    "#0x", 16) << "\n";
    os << "  rsp: " << fmt(state.rsp,    "#0x", 16) << "\n";
    os << "  rbp: " << fmt(state.rbp,    "#0x", 16) << "\n";
    os << "  r8 : " << fmt(state.r8,     "#0x", 16) << "\n";
    os << "  r9 : " << fmt(state.r9,     "#0x", 16) << "\n";
    os << "  r10: " << fmt(state.r10,    "#0x", 16) << "\n";
    os << "  r11: " << fmt(state.r11,    "#0x", 16) << "\n";
    os << "  r12: " << fmt(state.r12,    "#0x", 16) << "\n";
    os << "  r13: " << fmt(state.r13,    "#0x", 16) << "\n";
    os << "  r14: " << fmt(state.r14,    "#0x", 16) << "\n";
    os << "  r15: " << fmt(state.r15,    "#0x", 16) << "\n";
    os << "  flg: " << fmt(state.rflags, "#0x", 16) << "\n";

    return os;
}

void Exceptions::init() {
    if(env()->isrs) {
        // the PF exception is handled by RCTMux if we have an MMU
        bool want_pf = !env()->pedesc.has_mmu();
        auto funcs = reinterpret_cast<Exceptions::isr_func*>(env()->isrs);
        for(size_t i = 0; i < ARRAY_SIZE(exNames); ++i) {
            if(want_pf || i != 0xe)
                funcs[i] = handler;
        }
    }
}

void *Exceptions::handler(State *state) {
    Serial::get() << *state;

    env()->exit(1);
    return state;
}

}
