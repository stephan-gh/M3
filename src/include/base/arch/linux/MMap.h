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

#pragma once

#include <base/Common.h>
#include <base/KIF.h>

namespace m3lx {

enum MemType {
    TCU,
    TCUEps,
    Environment,
    StdRecvBuf,
    Custom,
};

void mmap_tcu(int fd, void *addr, size_t size, MemType type, uint perm);
void munmap_tcu(void *addr, size_t size);

}
