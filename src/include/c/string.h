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

#include <base/Common.h>

EXTERN_C int bcmp(const void *s1, const void *s2, size_t n);
EXTERN_C int memcmp(const void *mem1, const void *mem2, size_t count);
EXTERN_C void *memcpy(void *dest, const void *src, size_t len);
EXTERN_C void *memmove(void *dest, const void *src, size_t count);
EXTERN_C void *memset(void *addr, int value, size_t count);
EXTERN_C void memzero(void *addr, size_t count);

EXTERN_C char *strchr(const char *str, int ch);
EXTERN_C int strcmp(const char *str1, const char *str2);
EXTERN_C char *strcpy(char *dst, const char *src);
EXTERN_C size_t strlen(const char *s);
EXTERN_C char *strncat(char *str1, const char *str2, size_t count);
EXTERN_C int strncmp(const char *str1, const char *str2, size_t count);
EXTERN_C char *strncpy(char *to, const char *from, size_t count);
EXTERN_C char *strrchr(const char *str, int ch);
EXTERN_C char *strstr(const char *str1, const char *str2);
