/*
 * Copyright (C) 2022 Nils Asmussen, Barkhausen Institut
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

// "restrict" is a keyword in C, but not in C++. Thus, redefine it during the inclusion of headers.
// The problem occurs here, because we use the C++ compiler for some files in musl and have internal
// C headers of musl in the include path.
#ifdef __cplusplus
#    define restrict __restrict
#endif

#include <base/Common.h>

#include <m3/vfs/Dir.h>
#include <m3/vfs/File.h>

#undef restrict

EXTERN_C NORETURN void __m3c_exit(m3::Errors::Code status, bool abort);
EXTERN_C int __m3c_getpid();

EXTERN_C m3::Errors::Code __m3c_fstat(int fd, m3::FileInfo *info);
EXTERN_C m3::Errors::Code __m3c_stat(const char *pathname, m3::FileInfo *info);
EXTERN_C m3::Errors::Code __m3c_mkdir(const char *pathname, m3::mode_t mode);
EXTERN_C m3::Errors::Code __m3c_rmdir(const char *pathname);
EXTERN_C m3::Errors::Code __m3c_rename(const char *oldpath, const char *newpath);
EXTERN_C m3::Errors::Code __m3c_link(const char *oldpath, const char *newpath);
EXTERN_C m3::Errors::Code __m3c_unlink(const char *pathname);

EXTERN_C m3::Errors::Code __m3c_opendir(int fd, void **dir);
EXTERN_C m3::Errors::Code __m3c_readdir(void *dir, m3::Dir::Entry *entry);
EXTERN_C void __m3c_closedir(void *dir);

EXTERN_C m3::Errors::Code __m3c_chdir(const char *path);
EXTERN_C m3::Errors::Code __m3c_fchdir(int fd);
EXTERN_C m3::Errors::Code __m3c_getcwd(char *buf, size_t *size);

EXTERN_C m3::Errors::Code __m3c_open(const char *pathname, int flags, int *fd);
EXTERN_C m3::Errors::Code __m3c_read(int fd, void *buf, size_t *count);
EXTERN_C m3::Errors::Code __m3c_write(int fd, const void *buf, size_t *count);
EXTERN_C m3::Errors::Code __m3c_fflush(int fd);
EXTERN_C m3::Errors::Code __m3c_lseek(int fd, size_t *offset, int whence);
EXTERN_C m3::Errors::Code __m3c_ftruncate(int fd, size_t length);
EXTERN_C m3::Errors::Code __m3c_truncate(const char *pathname, size_t length);
EXTERN_C m3::Errors::Code __m3c_sync(int fd);
EXTERN_C bool __m3c_isatty(int fd);
EXTERN_C void __m3c_close(int fd);

typedef void (*waiter_fetch_cb)(void *p, int fd, uint fdevs);

EXTERN_C m3::Errors::Code __m3c_waiter_create(void **waiter);
EXTERN_C void __m3c_waiter_add(void *waiter, int fd, uint events);
EXTERN_C void __m3c_waiter_set(void *waiter, int fd, uint events);
EXTERN_C void __m3c_waiter_rem(void *waiter, int fd);
EXTERN_C void __m3c_waiter_wait(void *waiter);
EXTERN_C void __m3c_waiter_waitfor(void *waiter, uint64_t timeout);
EXTERN_C void __m3c_waiter_fetch(void *waiter, void *arg, waiter_fetch_cb cb);
EXTERN_C void __m3c_waiter_destroy(void *waiter);

enum CompatSock {
    INVALID,
    DGRAM,
    STREAM,
};

struct CompatEndpoint {
    uint32_t addr;
    uint16_t port;
};

EXTERN_C m3::Errors::Code __m3c_socket(CompatSock type, int *fd);
EXTERN_C m3::Errors::Code __m3c_get_local_ep(int fd, CompatSock type, CompatEndpoint *ep);
EXTERN_C m3::Errors::Code __m3c_get_remote_ep(int fd, CompatSock type, CompatEndpoint *ep);
EXTERN_C m3::Errors::Code __m3c_bind_dgram(int fd, const CompatEndpoint *ep);
EXTERN_C m3::Errors::Code __m3c_accept_stream(int port, int *cfd, CompatEndpoint *ep);
EXTERN_C m3::Errors::Code __m3c_connect(int fd, CompatSock type, const CompatEndpoint *ep);
EXTERN_C m3::Errors::Code __m3c_sendto(int fd, CompatSock type, const void *buf, size_t *len,
                                       const CompatEndpoint *dest);
EXTERN_C m3::Errors::Code __m3c_recvfrom(int fd, CompatSock type, void *buf, size_t *len,
                                         CompatEndpoint *ep);
EXTERN_C m3::Errors::Code __m3c_abort_stream(int fd);

EXTERN_C uint64_t __m3c_get_nanos();
EXTERN_C void __m3c_get_time(int *seconds, long *nanos);
EXTERN_C void __m3c_sleep(int *seconds, long *nanos);

EXTERN_C void __m3c_print_syscall_start(const char *name, long a, long b, long c, long d, long e,
                                        long f);
EXTERN_C void __m3c_print_syscall_end(const char *name, long res, long a, long b, long c, long d,
                                      long e, long f);
EXTERN_C void __m3c_print_syscall_trace(size_t idx, const char *name, long no, uint64_t start,
                                        uint64_t end);
