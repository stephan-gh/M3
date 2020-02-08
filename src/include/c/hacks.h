/*
 * Copyright (C) 2016, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <stdarg.h>

// TODO temporary hacks to make the compiler happy
// these are necessary because we include <functional> for std::function, but have no
// libc and libstdc++ yet

#define _GLIBCXX_CSTDIO
#define _GLIBCXX_CWCHAR
#define _GLIBCXX_CCTYPE
#define _GLIBCXX_CLOCALE

#define alloca __builtin_alloca

#if defined(__x86_64__)
typedef uintptr_t __UINTPTR_TYPE__;
#endif

typedef int mbstate_t;

typedef long off_t;

#ifndef ERANGE
#   define ERANGE      100
#endif
#ifndef ENOSYS
#   define ENOSYS      102
#endif
#define ENOTSUP     101
extern int errno;

#define SEEK_SET    0

#ifndef NULL
#   define NULL        (0)
#endif

#if defined(__cplusplus)

EXTERN_C int wmemcmp(const wchar_t *mem1, const wchar_t *mem2, unsigned long count);
EXTERN_C wchar_t *wmemcpy(wchar_t *dest, const wchar_t *src, unsigned long len);
EXTERN_C wchar_t *wmemmove(wchar_t *dest, const wchar_t *src, unsigned long count);
EXTERN_C wchar_t *wmemset(wchar_t *addr, wchar_t value, unsigned long count);
EXTERN_C wchar_t *wmemchr(const wchar_t *s, wchar_t c, unsigned long n);
EXTERN_C unsigned long wcslen(const wchar_t *s);

namespace std {

#define WEOF        -1
#define LC_NUMERIC  0

typedef ::mbstate_t mbstate_t;
typedef int int_type;
typedef int wint_t;

#if defined(__arm__) || defined(__riscv)

#if defined(__arm__)
typedef unsigned int _usize_t;
#else
typedef unsigned long _usize_t;
#endif

EXTERN_C long int strtol(const char *nptr, char **endptr, int base);
EXTERN_C long long int strtoll(const char *nptr, char **endptr, int base);

EXTERN_C unsigned long int strtoul(const char *nptr, char **endptr, int base);
EXTERN_C unsigned long long int strtoull(const char *nptr, char **endptr, int base);

EXTERN_C double strtod(const char *nptr, char **endptr);
EXTERN_C float strtof(const char *nptr, char **endptr);
EXTERN_C long double strtold(const char *nptr, char **endptr);

EXTERN_C int vsnprintf(char *str, _usize_t size, const char *format, va_list ap);
EXTERN_C int vswprintf(wchar_t *wcs, _usize_t maxlen, const wchar_t *format, va_list args);

EXTERN_C long wcstol(const wchar_t *nptr, wchar_t **endptr, int base);
EXTERN_C long long wcstoll(const wchar_t *nptr, wchar_t **endptr, int base);

EXTERN_C unsigned long wcstoul(const wchar_t *nptr, wchar_t **endptr, int base);
EXTERN_C unsigned long long wcstoull(const wchar_t *nptr, wchar_t **endptr, int base);

EXTERN_C double wcstod(const wchar_t *nptr, wchar_t **endptr);
EXTERN_C float wcstof(const wchar_t *nptr, wchar_t **endptr);
EXTERN_C long double wcstold(const wchar_t *nptr, wchar_t **endptr);
#endif

EXTERN_C char *setlocale(int category, const char *locale);

}

#endif
