/*
 * Copyright (C) 2023 Nils Asmussen, Barkhausen Institut
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

#include <base/KIF.h>
#include <base/Panic.h>
#include <base/arch/linux/IOCtl.h>

#include <sys/ioctl.h>

namespace m3lx {

// this is defined in linux/drivers/tcu/tcu.cc
static const ulong IOCTL_WAIT_ACT = 0x80087101;
static const ulong IOCTL_RGSTR_ACT = 0x40087102;
static const ulong IOCTL_TLB_INSERT = 0x40087103;
static const ulong IOCTL_UNREG_ACT = 0x40087104;
static const ulong IOCTL_NOOP = 0x00007105;

void tlb_insert_addr(uintptr_t addr, uint perm) {
    using namespace m3;

    // touch the memory first to cause a page fault, because the TCU-TLB miss handler in the Linux
    // kernel cannot deal with the request if the page isn't mapped.
    UNUSED uint8_t dummy;
    volatile uint8_t *virt_ptr = reinterpret_cast<uint8_t *>(addr);
    if(perm & KIF::Perm::W)
        *virt_ptr = 0;
    else
        dummy = *virt_ptr;

    size_t arg = (addr & ~PAGE_MASK) | perm;
    int res = ::ioctl(tcu_fd(), IOCTL_TLB_INSERT, arg);
    if(res != 0)
        panic("ioctl call TLB_INSERT failed\n"_cf);
}

}
