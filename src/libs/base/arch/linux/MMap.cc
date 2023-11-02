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

#include <base/arch/linux/MMap.h>
#include <base/Panic.h>

#include <sys/mman.h>

namespace m3lx {

void mmap_tcu(int fd, void *addr, size_t size, MemType type, uint perm) {
    using namespace m3;

    int prot = 0;
    if(perm & KIF::Perm::R)
        prot |= PROT_READ;
    if(perm & KIF::Perm::W)
        prot |= PROT_WRITE;
    if(mmap(addr, size, prot, MAP_SHARED | MAP_FIXED | MAP_SYNC, fd, type << 12) == MAP_FAILED)
        panic("mmap syscall failed\n"_cf);
}

void munmap_tcu(void *addr, size_t size) {
    munmap(addr, size);
}

}
