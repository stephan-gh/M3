/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

#include <base/time/Instant.h>

#include <m3/session/LoadGen.h>
#include <m3/stream/Standard.h>
#include <m3/vfs/File.h>
#include <m3/vfs/Dir.h>
#include <m3/vfs/VFS.h>
#include <m3/tiles/Activity.h>

#include "exceptions.h"
#include "fsapi.h"
#include "buffer.h"

class FSAPI_M3FS : public FSAPI {
    enum { MaxOpenFds = 16 };

    void checkFd(int fd) {
        if(!_fdMap[fd].is_valid())
            exitmsg("Using uninitialized file @ " << fd);
    }

public:
    explicit FSAPI_M3FS(bool data, bool stdio, m3::String const &prefix, m3::LoadGen::Channel *lgchan)
        : _data(data),
          _start(m3::CycleInstant::now()),
          _prefix(prefix),
          _fdMap(),
          _dirMap(),
          _lgchan_fd(-1),
          _lgchan(lgchan) {
        if(_lgchan) {
            open_args_t args = { 5, "/tmp/log.txt", O_WRONLY|O_TRUNC|O_CREAT, 0644 };
            open(&args, 0);
        }
        if(stdio) {
            _fdMap[m3::STDIN_FD].reset(m3::Activity::own().files()->get(m3::STDIN_FD));
            _fdMap[m3::STDOUT_FD].reset(m3::Activity::own().files()->get(m3::STDOUT_FD));
        }
    }

    virtual ~FSAPI_M3FS() {
        for(size_t i = 0; i < ARRAY_SIZE(_dirMap); ++i) {
            if(_dirMap[i])
                delete _dirMap[i];
        }
    }

    virtual void start() override {
        _start = m3::CycleInstant::now();
    }
    virtual void stop() override {
        auto end = m3::CycleInstant::now();
        m3::cerr << "Total time: " << end.duration_since(_start) << "\n";
    }

    virtual void checkpoint(int, int, bool) override {
        // TODO not implemented
    }

    virtual void waituntil(UNUSED const waituntil_args_t *args, int) override {
        m3::CPU::compute(args->timestamp);
    }

    virtual void open(const open_args_t *args, UNUSED int lineNo) override {
        if(args->fd != -1 && (_fdMap[args->fd].is_valid() || _dirMap[args->fd] != nullptr))
            exitmsg("Overwriting already used file/dir @ " << args->fd);

        try {
            if(args->flags & O_DIRECTORY) {
                auto dir = new m3::Dir(add_prefix(args->name), m3::FILE_R);
                _dirMap[args->fd] = dir;
            }
            else {
                auto nfile = m3::VFS::open(add_prefix(args->name),
                                           args->flags | (_data ? 0 : m3::FILE_NODATA));
                _fdMap[args->fd] = std::move(nfile);
            }
        }
        catch(const m3::Exception &e) {
            if(args->fd != -1)
               throw ReturnValueException(e.code(), args->fd, lineNo);
        }
    }

    virtual void close(const close_args_t *args, int ) override {
        if(_fdMap[args->fd].is_valid())
            _fdMap[args->fd].reset();
        else if(_dirMap[args->fd]) {
            delete _dirMap[args->fd];
            _dirMap[args->fd] = nullptr;
        }
        else if(args->fd == _lgchan_fd)
            _lgchan_fd = -1;
        else
            exitmsg("Using uninitialized file @ " << args->fd);
    }

    virtual void fsync(const fsync_args_t *, int ) override {
        // TODO not implemented
    }

    virtual ssize_t read(int fd, void *buffer, size_t size) override {
        checkFd(fd);
        try {
            char *buf = reinterpret_cast<char*>(buffer);
            while(size > 0) {
                ssize_t res = _fdMap[fd]->read(buf, size);
                if(res <= 0)
                    break;
                size -= static_cast<size_t>(res);
                buf += res;
            }
            return buf - reinterpret_cast<char*>(buffer);
        }
        catch(const m3::Exception &e) {
            return -e.code();
        }
    }

    virtual ssize_t write(int fd, const void *buffer, size_t size) override {
        checkFd(fd);
        return write_file(&*_fdMap[fd], buffer, size);
    }

    ssize_t write_file(m3::File *file, const void *buffer, size_t size) {
        try {
            file->write_all(buffer, size);
        }
        catch(const m3::Exception &e) {
            return -e.code();
        }
        return static_cast<ssize_t>(size);
    }

    virtual ssize_t pread(int fd, void *buffer, size_t size, off_t offset) override {
        checkFd(fd);
        _fdMap[fd]->seek(static_cast<size_t>(offset), M3FS_SEEK_SET);
        return read(fd, buffer, size);
    }

    virtual ssize_t pwrite(int fd, const void *buffer, size_t size, off_t offset) override {
        checkFd(fd);
        _fdMap[fd]->seek(static_cast<size_t>(offset), M3FS_SEEK_SET);
        return write(fd, buffer, size);
    }

    virtual void lseek(const lseek_args_t *args, UNUSED int lineNo) override {
        checkFd(args->fd);
        try {
            _fdMap[args->fd]->seek(static_cast<size_t>(args->offset), args->whence);
        }
        catch(...) {
            // ignore
            // throw ReturnValueException(res, args->offset, lineNo);
        }
    }

    virtual void ftruncate(const ftruncate_args_t *, int ) override {
        // TODO not implemented
    }

    template<class F>
    int get_result_of(F func) {
        int res = m3::Errors::NONE;
        try {
            func();
        }
        catch(const m3::Exception &e) {
            res = -e.code();
        }
        return res;
    }

    virtual void fstat(const fstat_args_t *args, UNUSED int lineNo) override {
        int res = get_result_of([this, &args] {
            m3::FileInfo info;
            if(_fdMap[args->fd].is_valid())
                _fdMap[args->fd]->stat(info);
            else if(_dirMap[args->fd])
                _dirMap[args->fd]->stat(info);
            else
                exitmsg("Using uninitialized file/dir @ " << args->fd);
        });

        if ((res == m3::Errors::NONE) != (args->err == 0))
            throw ReturnValueException(res, args->err, lineNo);
    }

    virtual void fstatat(const fstatat_args_t *args, UNUSED int lineNo) override {
        m3::FileInfo info;
        m3::Errors::Code res = m3::VFS::try_stat(add_prefix(args->name), info);
        if ((res == m3::Errors::NONE) != (args->err == 0))
            throw ReturnValueException(res, args->err, lineNo);
    }

    virtual void stat(const stat_args_t *args, UNUSED int lineNo) override {
        m3::FileInfo info;
        m3::Errors::Code res = m3::VFS::try_stat(add_prefix(args->name), info);
        if ((res == m3::Errors::NONE) != (args->err == 0))
            throw ReturnValueException(res, args->err, lineNo);
    }

    virtual void rename(const rename_args_t *args, int lineNo) override {
        int res = get_result_of([this, &args] {
            char tmpto[255];
            m3::VFS::rename(add_prefix(args->from), add_prefix_to(args->to, tmpto, sizeof(tmpto)));
        });
        if ((res == m3::Errors::NONE) != (args->err == 0))
            throw ReturnValueException(res, args->err, lineNo);
    }

    virtual void unlink(const unlink_args_t *args, UNUSED int lineNo) override {
        int res = get_result_of([this, &args] {
            m3::VFS::unlink(add_prefix(args->name));
        });
        if ((res == m3::Errors::NONE) != (args->err == 0))
            throw ReturnValueException(res, args->err, lineNo);
    }

    virtual void rmdir(const rmdir_args_t *args, UNUSED int lineNo) override {
        int res = get_result_of([this, &args] {
            m3::VFS::rmdir(add_prefix(args->name));
        });
        if ((res == m3::Errors::NONE) != (args->err == 0))
            throw ReturnValueException(res, args->err, lineNo);
    }

    virtual void mkdir(const mkdir_args_t *args, UNUSED int lineNo) override {
        int res = get_result_of([this, &args] {
            m3::VFS::mkdir(add_prefix(args->name), 0777 /*args->mode*/);
        });
        if ((res == m3::Errors::NONE) != (args->err == 0))
            throw ReturnValueException(res, args->err, lineNo);
    }

    virtual void sendfile(Buffer &buf, const sendfile_args_t *args, int lineNo) override {
        assert(args->offset == nullptr);

        if(args->out_fd == _lgchan_fd) {
            lgchansend(buf, args, lineNo);
            return;
        }

        checkFd(args->in_fd);
        checkFd(args->out_fd);
        char *rbuf = buf.readBuffer(Buffer::MaxBufferSize);
        size_t rem = args->count;
        while(rem > 0) {
            size_t amount = m3::Math::min(static_cast<size_t>(Buffer::MaxBufferSize), rem);

            ssize_t res = _fdMap[args->in_fd]->read(rbuf, amount);
            if(res <= 0)
                break;

            ssize_t wres = write_file(&*_fdMap[args->out_fd], rbuf, static_cast<size_t>(res));
            if(wres != res)
                throw ReturnValueException(static_cast<int>(wres), static_cast<int>(res), lineNo);

            rem -= static_cast<size_t>(res);
        }

        int expected = static_cast<int>(args->count - rem);
        if(expected != args->err)
            throw ReturnValueException(expected, args->err, lineNo);
    }

    virtual void getdents(const getdents_args_t *args, UNUSED int lineNo) override {
        if(_dirMap[args->fd] == nullptr)
            exitmsg("Using uninitialized dir @ " << args->fd);

        try {
            m3::Dir::Entry e;
            int i;
            // we don't check the result here because strace is often unable to determine the number of
            // fetched entries.
            if(args->count == 0 && _dirMap[args->fd]->readdir(e))
                ; //throw ReturnValueException(1, args->count, lineNo);
            else {
                for(i = 0; i < args->count && _dirMap[args->fd]->readdir(e); ++i)
                    ;
                //if(i != args->count)
                //    throw ReturnValueException(i, args->count, lineNo);
            }
        }
        catch(...) {
        }
    }

    virtual void createfile(const createfile_args_t *, int ) override {
        // TODO not implemented
    }

    virtual void accept(const accept_args_t *args, int lineNo) override {
        if(!_lgchan)
            throw NotSupportedException(lineNo);
        _lgchan->wait();
        _lgchan_fd = args->err;
    }
    virtual void recvfrom(Buffer &buf, const recvfrom_args_t *args, int lineNo) override {
        if(!_lgchan)
            throw NotSupportedException(lineNo);

        char *rbuf = buf.readBuffer(args->size);
        _lgchan->pull(rbuf, args->size);
    }
    virtual void writev(Buffer &buf, const writev_args_t *args, int lineNo) override {
        if(!_lgchan)
            throw NotSupportedException(lineNo);

        char *wbuf = buf.writeBuffer(args->size);
        _lgchan->push(wbuf, args->size);
    }
    void lgchansend(Buffer &buf, const sendfile_args_t *args, int lineNo) {
        if(!_lgchan)
            throw NotSupportedException(lineNo);

        checkFd(args->in_fd);

        char *rbuf = buf.readBuffer(Buffer::MaxBufferSize);
        size_t rem = args->count;
        while(rem > 0) {
            size_t amount = m3::Math::min(static_cast<size_t>(Buffer::MaxBufferSize), rem);

            ssize_t res = _fdMap[args->in_fd]->read(rbuf, amount);
            _lgchan->push(rbuf, static_cast<size_t>(res));

            rem -= static_cast<size_t>(res);
        }

        // there is always just one sendfile() call and it's the last data written to the socket
        _lgchan->reply();
    }

private:
    const char *add_prefix_to(const char *path, char *dst, size_t max) {
        if(_prefix.length() == 0 || strncmp(path, "/tmp/", 5) != 0)
            return path;

        m3::OStringStream os(dst, max);
        os << _prefix << (path + 5);
        return dst;
    }
    const char *add_prefix(const char *path) {
        static char tmp[255];
        return add_prefix_to(path, tmp, sizeof(tmp));
    }

    bool _wait;
    bool _data;
    m3::CycleInstant _start;
    const m3::String _prefix;
    m3::FileRef<m3::File> _fdMap[MaxOpenFds];
    m3::Dir *_dirMap[MaxOpenFds];
    fd_t _lgchan_fd;
    m3::LoadGen::Channel *_lgchan;
};
