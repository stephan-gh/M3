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

#pragma once

#include <base/Compiler.h>
#include <base/Types.h>

EXTERN_C void *malloc(size_t size);
EXTERN_C void *calloc(size_t n, size_t size);
EXTERN_C void *realloc(void *p, size_t size);
EXTERN_C void free(void *p);

EXTERN_C NORETURN void abort();
EXTERN_C NORETURN void exit(int code);
