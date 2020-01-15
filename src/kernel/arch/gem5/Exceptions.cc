/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/stream/Serial.h>
#include <base/Backtrace.h>
#include <base/Init.h>
#include <base/Panic.h>

#include <isr/ISR.h>

namespace kernel {

#if defined(__arm__)

static const char *exNames[] = {
    /* 0x00 */ "Reset",
    /* 0x01 */ "Undefined Instruction",
    /* 0x02 */ "Software Interrupt",
    /* 0x03 */ "Prefetch Abort",
    /* 0x04 */ "Data Abort",
    /* 0x05 */ "Reserved",
    /* 0x06 */ "IRQ",
    /* 0x07 */ "FIQ",
};

static m3::OStream &operator<<(m3::OStream &os, const m3::ISR::State &state) {
    os << "Interruption @ " << m3::fmt(state.pc, "p") << "\n";
    os << "  vector: ";
    if(state.vector < ARRAY_SIZE(exNames))
        os << exNames[state.vector];
    else
        os << "<unknown> (" << state.vector << ")";
    os << "\n";

    m3::Backtrace::print(os);

    os << "Registers:\n";
    for(size_t i = 0; i < ARRAY_SIZE(state.r); ++i)
        os << "   r" << m3::fmt(i, "0", 2) << ": " << m3::fmt(state.r[i], "#0x", 8) << "\n";
    os << "  cpsr: " << m3::fmt(state.cpsr, "#0x", 8) << "\n";
    os << "    lr: " << m3::fmt(state.lr, "#0x", 8) << "\n";
    return os;
}

#else

extern "C" m3::DTU::pte_t get_pte(uintptr_t virt, uint64_t perm);

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

static m3::OStream &operator<<(m3::OStream &os, const m3::ISR::State &state) {
    os << "Interruption @ " << m3::fmt(state.rip, "p");
    if(state.intrptNo == 0xe)
        os << " for address " << m3::fmt(getCR2(), "p");
    os << "\n  irq: ";
    if(state.intrptNo < ARRAY_SIZE(exNames))
        os << exNames[state.intrptNo];
    else if(state.intrptNo == 64)
        os << "DTU (" << state.intrptNo << ")";
    else
        os << "<unknown> (" << state.intrptNo << ")";
    os << "\n";

    m3::Backtrace::print(os);

    os << "  err: " << state.errorCode << "\n";
    os << "  rax: " << m3::fmt(state.rax,    "#0x", 16) << "\n";
    os << "  rbx: " << m3::fmt(state.rbx,    "#0x", 16) << "\n";
    os << "  rcx: " << m3::fmt(state.rcx,    "#0x", 16) << "\n";
    os << "  rdx: " << m3::fmt(state.rdx,    "#0x", 16) << "\n";
    os << "  rsi: " << m3::fmt(state.rsi,    "#0x", 16) << "\n";
    os << "  rdi: " << m3::fmt(state.rdi,    "#0x", 16) << "\n";
    os << "  rsp: " << m3::fmt(state.rsp,    "#0x", 16) << "\n";
    os << "  rbp: " << m3::fmt(state.rbp,    "#0x", 16) << "\n";
    os << "  r8 : " << m3::fmt(state.r8,     "#0x", 16) << "\n";
    os << "  r9 : " << m3::fmt(state.r9,     "#0x", 16) << "\n";
    os << "  r10: " << m3::fmt(state.r10,    "#0x", 16) << "\n";
    os << "  r11: " << m3::fmt(state.r11,    "#0x", 16) << "\n";
    os << "  r12: " << m3::fmt(state.r12,    "#0x", 16) << "\n";
    os << "  r13: " << m3::fmt(state.r13,    "#0x", 16) << "\n";
    os << "  r14: " << m3::fmt(state.r14,    "#0x", 16) << "\n";
    os << "  r15: " << m3::fmt(state.r15,    "#0x", 16) << "\n";
    os << "  flg: " << m3::fmt(state.rflags, "#0x", 16) << "\n";

    return os;
}

#endif

static void *irq_handler(m3::ISR::State *state) {
    m3::Serial::get() << *state;

    m3::Machine::shutdown();
    UNREACHED;
}

class ISR {
public:
    explicit ISR() {
        m3::ISR::init();
        for(size_t i = 0; i < m3::ISR::ISR_COUNT; ++i)
            m3::ISR::reg(i, irq_handler);
#if defined(__x86_64__)
        m3::ISR::reg(64, dtu_handler);
#endif
        m3::ISR::enable_irqs();
    }

#if defined(__x86_64__)
    static void handle_xlate(m3::DTU::reg_t xlate_req) {
        m3::DTU &dtu = m3::DTU::get();

        uintptr_t virt = xlate_req & ~PAGE_MASK;
        uint perm = (xlate_req >> 1) & 0xF;
        uint xferbuf = (xlate_req >> 5) & 0x7;

        m3::DTU::pte_t pte = get_pte(virt, perm);
        if(~(pte & 0xF) & perm)
            PANIC("Pagefault during PT walk for " << virt << " (PTE=" << m3::fmt(pte, "p") << ")");

        // tell DTU the result
        dtu.set_core_resp(pte | (xferbuf << 5));
    }

    static void *dtu_handler(m3::ISR::State *state) {
        m3::DTU &dtu = m3::DTU::get();

        // translation request from DTU?
        m3::DTU::reg_t core_req = dtu.get_core_req();
        if(core_req) {
            if(core_req & 0x1)
                PANIC("Unexpected foreign receive: " << m3::fmt(core_req, "x"));
            // acknowledge the translation
            dtu.set_core_req(0);
            handle_xlate(core_req);
        }
        return state;
    }
#endif

    static ISR irqs;
};

ISR ISR::irqs;

}
