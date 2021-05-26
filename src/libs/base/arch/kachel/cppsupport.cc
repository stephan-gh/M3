/*
 * Copyright (C) 2016-2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/Common.h>
#include <base/mem/Heap.h>
#include <base/Machine.h>
#include <base/Panic.h>
#include <functional>

using namespace m3;

namespace std {
WEAK void __throw_bad_function_call() {
    PANIC("bad function call");
}
}

EXTERN_C void *malloc(size_t size) {
    return heap_alloc(size);
}

EXTERN_C void *calloc(size_t n, size_t size) {
    return heap_calloc(n, size);
}

EXTERN_C void *realloc(void *p, size_t size) {
    return heap_realloc(p, size);
}

EXTERN_C void free(void *p) {
    return heap_free(p);
}

EXTERN_C void *__libc_malloc(size_t size) __attribute__((__weak__, __alias__("malloc")));
EXTERN_C void *__libc_calloc(size_t n, size_t size) __attribute__((__weak__, __alias__("calloc")));
EXTERN_C void *__libc_realloc(void *p, size_t size) __attribute__((__weak__, __alias__("realloc")));
EXTERN_C void __libc_free(void *p) __attribute__((__weak__, __alias__("free")));

#ifndef NDEBUG
void __assert_fail(const char *expr, const char *file, int line, const char *func) {
    m3::Serial::get() << "assertion \"" << expr << "\" failed in " << func << " in "
                      << file << ":" << line << "\n";
    exit(1);
    /* NOTREACHED */
}
#endif

#if defined(__arm__)
EXTERN_C void __sync_synchronize() {
}
#endif
