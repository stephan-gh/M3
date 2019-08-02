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
#include <base/Heap.h>
#include <base/Machine.h>
#include <base/Panic.h>
#include <functional>

using namespace m3;

struct GlobalObj {
    void (*f)(void*);
    void *p;
    void *d;
};

static constexpr size_t MAX_EXIT_FUNCS = 8;

static size_t exit_count = 0;
static GlobalObj exit_funcs[MAX_EXIT_FUNCS];

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

EXTERN_C int __cxa_atexit(void (*f)(void *),void *p,void *d) {
    if(exit_count >= MAX_EXIT_FUNCS)
        return -1;

    exit_funcs[exit_count].f = f;
    exit_funcs[exit_count].p = p;
    exit_funcs[exit_count].d = d;
    exit_count++;
    return 0;
}

EXTERN_C void __cxa_finalize(void *) {
    for(ssize_t i = static_cast<ssize_t>(exit_count) - 1; i >= 0; i--)
        exit_funcs[i].f(exit_funcs[i].p);
}

#ifndef NDEBUG
void __assert_failed(const char *expr, const char *file, const char *func, int line) {
    m3::Serial::get() << "assertion \"" << expr << "\" failed in " << func << " in "
                      << file << ":" << line << "\n";
    exit(1);
    /* NOTREACHED */
}
#endif

// for __verbose_terminate_handler from libsupc++
void *stderr;
EXTERN_C int fputs(const char *str, void *) {
    m3::Serial::get() << str;
    return 0;
}
EXTERN_C int fputc(int c, void *) {
    m3::Serial::get().write(c);
    return -1;
}
EXTERN_C size_t fwrite(const void *str, UNUSED size_t size, size_t nmemb, void *) {
    assert(size == 1);
    const char *s = reinterpret_cast<const char*>(str);
    auto &ser = m3::Serial::get();
    while(nmemb-- > 0)
        ser.write(*s++);
    return 0;
}
