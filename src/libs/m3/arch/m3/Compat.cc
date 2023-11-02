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

#include <base/arch/m3/Init.h>

#include <m3/Compat.h>
#include <m3/net/TcpSocket.h>
#include <m3/net/UdpSocket.h>
#include <m3/session/Network.h>
#include <m3/vfs/VFS.h>
#include <m3/vfs/Waiter.h>

EXTERN_C int __m3c_getpid() {
    // + 1, because our ids start with 0, but pid 0 is special
    return m3::Activity::own().id() + 1;
}

EXTERN_C m3::Errors::Code __m3c_fstat(int fd, m3::FileInfo *info) {
    try {
        auto file = m3::Activity::own().files()->get(fd);
        return file->try_stat(*info);
    }
    catch(const m3::Exception &e) {
        return e.code();
    }
}
EXTERN_C m3::Errors::Code __m3c_stat(const char *pathname, m3::FileInfo *info) {
    return m3::VFS::try_stat(pathname, *info);
}

EXTERN_C m3::Errors::Code __m3c_mkdir(const char *pathname, m3::mode_t mode) {
    return m3::VFS::try_mkdir(pathname, mode);
}
EXTERN_C m3::Errors::Code __m3c_rmdir(const char *pathname) {
    return m3::VFS::try_rmdir(pathname);
}

EXTERN_C m3::Errors::Code __m3c_rename(const char *oldpath, const char *newpath) {
    return m3::VFS::try_rename(oldpath, newpath);
}
EXTERN_C m3::Errors::Code __m3c_link(const char *oldpath, const char *newpath) {
    return m3::VFS::try_link(oldpath, newpath);
}
EXTERN_C m3::Errors::Code __m3c_unlink(const char *pathname) {
    return m3::VFS::try_unlink(pathname);
}

EXTERN_C m3::Errors::Code __m3c_opendir(int fd, void **dir) {
    try {
        m3::Activity::own().files()->get(fd);
        *dir = new m3::Dir(fd);
        return m3::Errors::SUCCESS;
    }
    catch(const m3::Exception &e) {
        return e.code();
    }
}
EXTERN_C m3::Errors::Code __m3c_readdir(void *dir, m3::Dir::Entry *entry) {
    try {
        if(!static_cast<m3::Dir *>(dir)->readdir(*entry))
            return m3::Errors::END_OF_FILE;
        return m3::Errors::SUCCESS;
    }
    catch(const m3::Exception &e) {
        return e.code();
    }
}
EXTERN_C void __m3c_closedir(void *dir) {
    delete static_cast<m3::Dir *>(dir);
}

EXTERN_C m3::Errors::Code __m3c_chdir(const char *path) {
    try {
        m3::VFS::set_cwd(path);
        return m3::Errors::SUCCESS;
    }
    catch(const m3::Exception &e) {
        return e.code();
    }
}
EXTERN_C m3::Errors::Code __m3c_fchdir(int fd) {
    try {
        m3::VFS::set_cwd(fd);
        return m3::Errors::SUCCESS;
    }
    catch(const m3::Exception &e) {
        return e.code();
    }
}
EXTERN_C m3::Errors::Code __m3c_getcwd(char *buf, size_t *size) {
    const char *cwd = m3::VFS::cwd();
    size_t len = strlen(cwd);
    if(len + 1 > *size)
        return m3::Errors::NO_SPACE;
    strcpy(buf, cwd);
    *size = len;
    return m3::Errors::SUCCESS;
}

EXTERN_C m3::Errors::Code __m3c_open(const char *pathname, int flags, int *fd) {
    try {
        *fd = m3::VFS::open(pathname, flags).release()->fd();
        return m3::Errors::SUCCESS;
    }
    catch(const m3::Exception &e) {
        return e.code();
    }
}
EXTERN_C m3::Errors::Code __m3c_read(int fd, void *buf, size_t *count) {
    try {
        auto file = m3::Activity::own().files()->get(fd);
        if(auto res = file->read(buf, *count)) {
            *count = res.unwrap();
            return m3::Errors::SUCCESS;
        }
        return m3::Errors::WOULD_BLOCK;
    }
    catch(const m3::Exception &e) {
        return e.code();
    }
}
EXTERN_C m3::Errors::Code __m3c_write(int fd, const void *buf, size_t *count) {
    try {
        auto file = m3::Activity::own().files()->get(fd);
        // use write_all here, because some tools seem to expect that write can't write less than
        // requested and we don't really lose anything by calling write_all instead of write.
        if(auto res = file->write_all(buf, *count)) {
            *count = res.unwrap();
            return m3::Errors::SUCCESS;
        }
        return m3::Errors::WOULD_BLOCK;
    }
    catch(const m3::Exception &e) {
        return e.code();
    }
}
EXTERN_C m3::Errors::Code __m3c_fflush(int fd) {
    try {
        m3::File *file = m3::Activity::own().files()->get(fd);
        file->flush();
        return m3::Errors::SUCCESS;
    }
    catch(const m3::Exception &e) {
        return e.code();
    }
}
EXTERN_C m3::Errors::Code __m3c_lseek(int fd, size_t *offset, int whence) {
    try {
        auto file = m3::Activity::own().files()->get(fd);
        auto res = file->seek(static_cast<size_t>(*offset), whence);
        *offset = res;
        return m3::Errors::SUCCESS;
    }
    catch(const m3::Exception &e) {
        return e.code();
    }
}
EXTERN_C m3::Errors::Code __m3c_ftruncate(int fd, size_t length) {
    try {
        auto file = m3::Activity::own().files()->get(fd);
        file->truncate(length);
        return m3::Errors::SUCCESS;
    }
    catch(const m3::Exception &e) {
        return e.code();
    }
}
EXTERN_C m3::Errors::Code __m3c_truncate(const char *pathname, size_t length) {
    try {
        auto file = m3::VFS::open(pathname, m3::FILE_W);
        file->truncate(length);
        return m3::Errors::SUCCESS;
    }
    catch(const m3::Exception &e) {
        return e.code();
    }
}
EXTERN_C m3::Errors::Code __m3c_sync(int fd) {
    try {
        auto file = m3::Activity::own().files()->get(fd);
        file->sync();
        return m3::Errors::SUCCESS;
    }
    catch(const m3::Exception &e) {
        return e.code();
    }
}

EXTERN_C bool __m3c_isatty(int fd) {
    // try to use the get_tmode operation; only works for vterm
    auto file = m3::Activity::own().files()->get(fd);
    m3::File::TMode mode;
    return file->try_get_tmode(&mode) == m3::Errors::SUCCESS;
}

EXTERN_C void __m3c_close(int fd) {
    m3::Activity::own().files()->remove(fd);
}

EXTERN_C m3::Errors::Code __m3c_waiter_create(void **waiter) {
    *waiter = new m3::FileWaiter();
    return m3::Errors::SUCCESS;
}
EXTERN_C void __m3c_waiter_add(void *waiter, int fd, uint events) {
    static_cast<m3::FileWaiter *>(waiter)->add(fd, events);
}
EXTERN_C void __m3c_waiter_set(void *waiter, int fd, uint events) {
    static_cast<m3::FileWaiter *>(waiter)->set(fd, events);
}
EXTERN_C void __m3c_waiter_rem(void *waiter, int fd) {
    static_cast<m3::FileWaiter *>(waiter)->remove(fd);
}
EXTERN_C void __m3c_waiter_wait(void *waiter) {
    static_cast<m3::FileWaiter *>(waiter)->wait();
}
EXTERN_C void __m3c_waiter_waitfor(void *waiter, uint64_t timeout) {
    static_cast<m3::FileWaiter *>(waiter)->wait_for(m3::TimeDuration::from_nanos(timeout));
}
EXTERN_C void __m3c_waiter_fetch(void *waiter, void *arg, waiter_fetch_cb cb) {
    static_cast<m3::FileWaiter *>(waiter)->foreach_ready([arg, cb](int fd, uint fevs) {
        cb(arg, fd, fevs);
    });
}
EXTERN_C void __m3c_waiter_destroy(void *waiter) {
    delete static_cast<m3::FileWaiter *>(waiter);
}

static m3::Network *netmng = nullptr;

EXTERN_C m3::Errors::Code __m3c_init_netmng(const char *name) {
    if(!netmng) {
        try {
            netmng = new m3::Network(name);
        }
        catch(const m3::Exception &e) {
            return e.code();
        }
    }
    return m3::Errors::SUCCESS;
}

static m3::Errors::Code init_netmng() {
    return __m3c_init_netmng("net");
}

static m3::Socket *get_socket(int fd) {
    try {
        return static_cast<m3::Socket *>(m3::Activity::own().files()->get(fd));
    }
    catch(const m3::Exception &) {
        return nullptr;
    }
}

EXTERN_C m3::Errors::Code __m3c_socket(CompatSock type, int *fd) {
    m3::Errors::Code res;
    if((res = init_netmng()) < 0)
        return res;

    m3::File *file = nullptr;
    try {
        switch(type) {
            case CompatSock::STREAM: file = m3::TcpSocket::create(*netmng).release(); break;
            case CompatSock::DGRAM: file = m3::UdpSocket::create(*netmng).release(); break;
            default: UNREACHED;
        }
    }
    catch(const m3::Exception &e) {
        return e.code();
    }
    *fd = file->fd();
    return m3::Errors::SUCCESS;
}

EXTERN_C m3::Errors::Code __m3c_get_local_ep(int fd, UNUSED CompatSock type, CompatEndpoint *ep) {
    auto *s = get_socket(fd);
    if(!s)
        return m3::Errors::BAD_FD;
    ep->addr = s->local_endpoint().addr.addr();
    ep->port = s->local_endpoint().port;
    return m3::Errors::SUCCESS;
}

EXTERN_C m3::Errors::Code __m3c_get_remote_ep(int fd, UNUSED CompatSock type, CompatEndpoint *ep) {
    auto *s = get_socket(fd);
    if(!s)
        return m3::Errors::BAD_FD;
    ep->addr = s->remote_endpoint().addr.addr();
    ep->port = s->remote_endpoint().port;
    return m3::Errors::SUCCESS;
}

EXTERN_C m3::Errors::Code __m3c_bind_dgram(int fd, const CompatEndpoint *ep) {
    auto *s = get_socket(fd);
    if(!s)
        return m3::Errors::BAD_FD;
    static_cast<m3::UdpSocket *>(s)->bind(ep->port);
    return m3::Errors::SUCCESS;
}

EXTERN_C m3::Errors::Code __m3c_accept_stream(int port, int *cfd, CompatEndpoint *ep) {
    // create a new socket for the to-be-accepted client
    m3::Errors::Code res = __m3c_socket(CompatSock::STREAM, cfd);
    if(res != m3::Errors::SUCCESS)
        return res;

    // put the socket into listen mode
    auto *cs = static_cast<m3::TcpSocket *>(get_socket(*cfd));
    assert(cs != nullptr);
    try {
        cs->listen(port);
    }
    catch(const m3::Exception &e) {
        __m3c_close(*cfd);
        return e.code();
    }

    // accept the client connection
    cs->accept(nullptr);

    ep->addr = cs->remote_endpoint().addr.addr();
    ep->port = cs->remote_endpoint().port;
    return m3::Errors::SUCCESS;
}

EXTERN_C m3::Errors::Code __m3c_connect(int fd, UNUSED CompatSock type, const CompatEndpoint *ep) {
    auto *s = get_socket(fd);
    if(!s)
        return m3::Errors::BAD_FD;

    try {
        s->connect(m3::Endpoint(m3::IpAddr(ep->addr), ep->port));
        return m3::Errors::SUCCESS;
    }
    catch(const m3::Exception &e) {
        return e.code();
    }
}

EXTERN_C m3::Errors::Code __m3c_sendto(int fd, CompatSock type, const void *buf, size_t *len,
                                       const CompatEndpoint *dest) {
    auto *s = get_socket(fd);
    if(!s)
        return m3::Errors::BAD_FD;

    try {
        m3::Option<size_t> res = m3::None;
        switch(type) {
            case CompatSock::STREAM: res = s->send(buf, *len); break;
            default:
            case CompatSock::DGRAM: {
                m3::Endpoint m3ep(m3::IpAddr(dest->addr), dest->port);
                res = static_cast<m3::UdpSocket *>(s)->send_to(buf, *len, m3ep);
                break;
            }
        }
        if(auto r = res) {
            *len = r.unwrap();
            return m3::Errors::SUCCESS;
        }
        return m3::Errors::WOULD_BLOCK;
    }
    catch(const m3::Exception &e) {
        return e.code();
    }
    UNREACHED;
}

EXTERN_C m3::Errors::Code __m3c_recvfrom(int fd, CompatSock type, void *buf, size_t *len,
                                         CompatEndpoint *ep) {
    auto *s = get_socket(fd);
    if(!s)
        return m3::Errors::BAD_FD;

    try {
        switch(type) {
            case CompatSock::STREAM:
                if(auto recv_res = s->recv(buf, *len)) {
                    *len = recv_res.unwrap();
                    ep->addr = s->remote_endpoint().addr.addr();
                    ep->port = s->remote_endpoint().port;
                }
                else
                    return m3::Errors::WOULD_BLOCK;
                break;
            default:
            case CompatSock::DGRAM: {
                auto udp_sock = static_cast<m3::UdpSocket *>(s);
                if(auto recv_res = udp_sock->recv_from(buf, *len)) {
                    *len = recv_res.unwrap().first;
                    ep->addr = recv_res.unwrap().second.addr.addr();
                    ep->port = recv_res.unwrap().second.port;
                }
                else
                    return m3::Errors::WOULD_BLOCK;
                break;
            }
        }
        return m3::Errors::SUCCESS;
    }
    catch(const m3::Exception &e) {
        return e.code();
    }
}

EXTERN_C m3::Errors::Code __m3c_abort_stream(int fd) {
    auto *s = get_socket(fd);
    if(!s)
        return m3::Errors::BAD_FD;
    static_cast<m3::TcpSocket *>(s)->abort();
    return m3::Errors::SUCCESS;
}

EXTERN_C uint64_t __m3c_get_nanos() {
    return m3::TimeInstant::now().elapsed().as_nanos();
}

EXTERN_C void __m3c_get_time(int *seconds, long *nanos) {
    auto now = m3::TimeInstant::now();
    *seconds = now.as_nanos() / 1000000000;
    *nanos = static_cast<long>(now.as_nanos() - (static_cast<ulong>(*seconds) * 1000000000));
}

EXTERN_C void __m3c_sleep(int *seconds, long *nanos) {
    auto start = m3::TimeInstant::now();

    uint64_t allnanos = static_cast<uint64_t>(*nanos) + static_cast<ulong>(*seconds) * 1000000000;
    m3::OwnActivity::sleep_for(m3::TimeDuration::from_nanos(allnanos));

    auto duration = m3::TimeInstant::now().duration_since(start);
    auto remaining = m3::TimeDuration::from_nanos(allnanos) - duration;
    *seconds = remaining.as_secs();
    *nanos = static_cast<long>(remaining.as_nanos() - (static_cast<ulong>(*seconds) * 1000000000));
}

EXTERN_C void __m3c_print_syscall_start(const char *name, long a, long b, long c, long d, long e,
                                        long f) {
    using namespace m3;
    char syscbuf[256];
    OStringStream os(syscbuf, sizeof(syscbuf));
    format_to(os, "{}({}, {}, {}, {}, {}, {})...\n"_cf, name, a, b, c, d, e, f);
    Machine::write(os.str(), os.length());
}

EXTERN_C void __m3c_print_syscall_end(const char *name, long res, long a, long b, long c, long d,
                                      long e, long f) {
    using namespace m3;
    char buf[256];
    OStringStream os(buf, sizeof(buf));
    format_to(os, "{}({}, {}, {}, {}, {}, {}) -> {}\n"_cf, name, a, b, c, d, e, f, res);
    Machine::write(os.str(), os.length());
}

EXTERN_C void __m3c_print_syscall_trace(size_t idx, const char *name, long no, uint64_t start,
                                        uint64_t end) {
    using namespace m3;
    char buf[256];
    OStringStream os(buf, sizeof(buf));
    format_to(os, "[{: <3} {}({}) {:011} {:011}\n"_cf, idx, name, no, start, end);
    Machine::write(os.str(), os.length());
}
