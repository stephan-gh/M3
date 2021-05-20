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
    namespace placeholders {
        // two are enough for our purposes
        const _Placeholder<1> _1{};
        const _Placeholder<2> _2{};
    }
}

namespace std {
void __throw_length_error(char const *s) {
    PANIC(s);
}

void __throw_bad_alloc() {
    PANIC("bad alloc");
}

void __throw_bad_function_call() {
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

EXTERN_C void *__libc_calloc(size_t n, size_t size) __attribute__((__weak__, __alias__("calloc")));

#ifndef NDEBUG
void __assert_fail(const char *expr, const char *file, int line, const char *func) {
    m3::Serial::get() << "assertion \"" << expr << "\" failed in " << func << " in "
                      << file << ":" << line << "\n";
    exit(1);
    /* NOTREACHED */
}
#endif

// for __verbose_terminate_handler from libsupc++
void *stderr;
EXTERN_C WEAK int fputs(const char *str, void *) {
    m3::Serial::get() << str;
    return 0;
}
EXTERN_C WEAK int fputc(int c, void *) {
    m3::Serial::get().write(c);
    return -1;
}
EXTERN_C WEAK size_t fwrite(const void *str, UNUSED size_t size, size_t nmemb, void *) {
    assert(size == 1);
    const char *s = reinterpret_cast<const char*>(str);
    auto &ser = m3::Serial::get();
    while(nmemb-- > 0)
        ser.write(*s++);
    return 0;
}
