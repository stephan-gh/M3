/*
 * Copyright (C) 2016-2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <isr/ISR.h>

EXTERN_C void *isr_stack;

// Our ISRs
EXTERN_C void isr_0();
EXTERN_C void isr_1();
EXTERN_C void isr_2();
EXTERN_C void isr_3();
EXTERN_C void isr_4();
EXTERN_C void isr_5();
EXTERN_C void isr_6();
EXTERN_C void isr_7();
EXTERN_C void isr_8();
EXTERN_C void isr_9();
EXTERN_C void isr_10();
EXTERN_C void isr_11();
EXTERN_C void isr_12();
EXTERN_C void isr_13();
EXTERN_C void isr_14();
EXTERN_C void isr_15();
EXTERN_C void isr_16();
// for the DTU
EXTERN_C void isr_64();
// the handler for a other interrupts
EXTERN_C void isr_null();

namespace m3 {

Exceptions::isr_func ISR::isrs[ISR_COUNT];

ISR::Desc ISRBase::gdt[GDT_ENTRY_COUNT];
ISR::Desc64 ISRBase::idt[ISR_COUNT];
ISR::TSS ISRBase::tss ALIGNED(PAGE_SIZE);

void *ISR::handler(m3::Exceptions::State *state) {
    return isrs[state->intrptNo](state);
}

void ISR::init() {
    // setup GDT
    DescTable gdtTable;
    gdtTable.offset = reinterpret_cast<uintptr_t>(gdt);
    gdtTable.size = GDT_ENTRY_COUNT * sizeof(Desc) - 1;

    // code+data
    set_desc(gdt + SEG_KCODE, 0, ~0UL >> PAGE_BITS, Desc::GRANU_PAGES, Desc::CODE_XR, Desc::DPL_KERNEL);
    set_desc(gdt + SEG_KDATA, 0, ~0UL >> PAGE_BITS, Desc::GRANU_PAGES, Desc::DATA_RW, Desc::DPL_KERNEL);
    set_desc(gdt + SEG_UCODE, 0, ~0UL >> PAGE_BITS, Desc::GRANU_PAGES, Desc::CODE_XR, Desc::DPL_USER);
    set_desc(gdt + SEG_UDATA, 0, ~0UL >> PAGE_BITS, Desc::GRANU_PAGES, Desc::DATA_RW, Desc::DPL_USER);
    set_tss(gdt, &tss, reinterpret_cast<uintptr_t>(&isr_stack));

    // now load GDT and TSS
    load_gdt(&gdtTable);
    load_tss(SEG_TSS * sizeof(Desc));

    // setup the idt
    set_idt(0, isr_0, Desc::DPL_KERNEL);
    set_idt(1, isr_1, Desc::DPL_KERNEL);
    set_idt(2, isr_2, Desc::DPL_KERNEL);
    set_idt(3, isr_3, Desc::DPL_KERNEL);
    set_idt(4, isr_4, Desc::DPL_KERNEL);
    set_idt(5, isr_5, Desc::DPL_KERNEL);
    set_idt(6, isr_6, Desc::DPL_KERNEL);
    set_idt(7, isr_7, Desc::DPL_KERNEL);
    set_idt(8, isr_8, Desc::DPL_KERNEL);
    set_idt(9, isr_9, Desc::DPL_KERNEL);
    set_idt(10, isr_10, Desc::DPL_KERNEL);
    set_idt(11, isr_11, Desc::DPL_KERNEL);
    set_idt(12, isr_12, Desc::DPL_KERNEL);
    set_idt(13, isr_13, Desc::DPL_KERNEL);
    set_idt(14, isr_14, Desc::DPL_KERNEL);
    set_idt(15, isr_15, Desc::DPL_KERNEL);
    set_idt(16, isr_16, Desc::DPL_KERNEL);

    // all other interrupts
    for(size_t i = 17; i < 63; i++)
        set_idt(i, isr_null, Desc::DPL_KERNEL);

    // DTU interrupts
    set_idt(64, isr_64, Desc::DPL_KERNEL);

    for(size_t i = 0; i < ISR_COUNT; ++i)
        reg(i, null_handler);

    // now we can use our idt
    DescTable tbl;
    tbl.offset = reinterpret_cast<uintptr_t>(idt);
    tbl.size = sizeof(idt) - 1;
    load_idt(&tbl);
}

void ISRBase::set_desc(Desc *d, uintptr_t address, size_t limit, uint8_t granu,
                         uint8_t type, uint8_t dpl) {
    d->addrLow = address & 0xFFFF;
    d->addrMiddle = (address >> 16) & 0xFF;
    d->limitLow = limit & 0xFFFF;
    d->addrHigh = ((address & 0xFF000000) >> 16) | ((limit >> 16) & 0xF) |
        Desc::BITS_64 | Desc::SIZE_16 | granu;
    d->present = 1;
    d->dpl = dpl;
    d->type = type;
}

void ISRBase::set_desc64(Desc *d, uintptr_t address, size_t limit, uint8_t granu,
                           uint8_t type, uint8_t dpl) {
    Desc64 *d64 = reinterpret_cast<Desc64*>(d);
    set_desc(d64,address,limit,granu,type,dpl);
    d64->addrUpper = address >> 32;
}

void ISRBase::set_idt(size_t number, entry_func handler, uint8_t dpl) {
    Desc64 *e = idt + number;
    e->type = Desc::SYS_INTR_GATE;
    e->dpl = dpl;
    e->present = number != 2 && number != 15; /* reserved by intel */
    e->addrLow = SEG_KCODE << 3;
    e->addrHigh = (reinterpret_cast<uintptr_t>(handler) >> 16) & 0xFFFF;
    e->limitLow = reinterpret_cast<uintptr_t>(handler) & 0xFFFF;
    e->addrUpper = reinterpret_cast<uintptr_t>(handler) >> 32;
}

void ISRBase::set_tss(Desc *gdt, TSS *tss, uintptr_t kstack) {
    /* an invalid offset for the io-bitmap => not loaded yet */
    tss->ioMapOffset = 104 + 16;
    tss->rsp0 = kstack;
    set_desc64(gdt + SEG_TSS, reinterpret_cast<uintptr_t>(tss), sizeof(TSS) - 1,
        Desc::GRANU_BYTES, Desc::SYS_TSS, Desc::DPL_KERNEL);
}

}
