/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019 Nils Asmussen, Barkhausen Institut
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

#include <m3/session/M3FS.h>
#include <m3/stream/FStream.h>
#include <m3/vfs/VFS.h>

namespace m3 {

FStream::FStream(fd_t fd, int perms, size_t bufsize, uint flags)
    : IStream(),
      OStream(),
      _fd(fd),
      _rbuf(new File::Buffer((perms & FILE_R) ? bufsize : 0)),
      _wbuf(new File::Buffer((perms & FILE_W) ? bufsize : 0)),
      _flags(FL_DEL_BUF | flags) {
    if(!file())
        _state = FL_ERROR;
}

FStream::FStream(const char *filename, int perms, size_t bufsize)
    : FStream(filename, bufsize, bufsize, perms) {
}

FStream::FStream(const char *filename, size_t rsize, size_t wsize, int perms)
    : IStream(),
      OStream(),
      _fd(VFS::open(filename, get_perms(perms)).release()->fd()),
      _rbuf(new File::Buffer((perms & FILE_R) ? rsize : 0)),
      _wbuf(new File::Buffer((perms & FILE_W) ? wsize : 0)),
      _flags(FL_DEL_BUF | FL_DEL_FILE) {
}

FStream::~FStream() {
    if(Activity::own().files()->exists(_fd)) {
        try {
            flush();
        }
        catch(...) {
            // ignore
        }
    }

    if(!(_flags & FL_DEL_BUF)) {
        if(_rbuf)
            _rbuf->buffer = nullptr;
        if(_wbuf)
            _wbuf->buffer = nullptr;
    }

    if((_flags & FL_DEL_FILE)) {
        try {
            Activity::own().files()->remove(_fd);
        }
        catch(...) {
            // ignore
        }
    }
}

void FStream::set_error(Option<size_t> res) {
    if(res.is_none())
        _state |= FL_ERROR;
    else if(res.unwrap() == 0)
        _state |= FL_EOF;
}

Option<size_t> FStream::read(void *dst, size_t count) {
    if(bad())
        return Some(size_t(0));

    // ensure that our write-buffer is empty
    // TODO maybe it's better to have just one buffer for both and track dirty regions?
    flush();

    // use the unbuffered read, if the buffer is smaller
    if(_rbuf->empty() && count > _rbuf->size) {
        auto res = file()->read(dst, count);
        set_error(res);
        return res;
    }

    if(!_rbuf->buffer) {
        _state |= FL_ERROR;
        return Some(size_t(0));
    }

    size_t total = 0;
    char *buf = reinterpret_cast<char *>(dst);
    File *f = file();
    while(count > 0) {
        auto res = _rbuf->read(f, buf + total, count);
        set_error(res);
        if(res.is_none())
            return total == 0 ? None : Some(total);

        size_t read = res.unwrap();
        if(read == 0)
            break;
        total += read;
        count -= read;
    }

    return Some(total);
}

void FStream::flush() {
    File *f = file();
    if(_wbuf && f) {
        _wbuf->flush(f);
        f->flush();
    }
}

size_t FStream::seek(size_t offset, int whence) {
    if(error())
        return 0;

    if(whence != M3FS_SEEK_CUR || offset != 0) {
        // TODO for simplicity, we always flush the write-buffer if we're changing the position
        flush();
    }

    // on relative seeks, take our position within the buffer into account
    if(whence == M3FS_SEEK_CUR)
        offset -= _rbuf->cur - _rbuf->pos;

    size_t res = file()->seek(offset, whence);
    _rbuf->invalidate();
    return res;
}

Option<size_t> FStream::write(const void *src, size_t count) {
    if(bad())
        return Some(size_t(0));

    // use the unbuffered write, if the buffer is smaller
    if(_wbuf->empty() && count > _wbuf->size) {
        auto res = file()->write(src, count);
        set_error(res);
        return res;
    }

    if(!_wbuf->buffer) {
        _state |= FL_ERROR;
        return Some(size_t(0));
    }

    const char *buf = reinterpret_cast<const char *>(src);
    size_t total = 0;
    File *f = file();
    while(count > 0) {
        auto res = _wbuf->write(f, buf + total, count);
        set_error(res);
        if(res.is_none())
            return total == 0 ? None : Some(total);

        size_t written = res.unwrap();
        if(written == 0)
            break;

        total += written;
        count -= written;

        if(((_flags & FL_LINE_BUF) && buf[total - 1] == '\n'))
            flush();
        else if(count)
            _wbuf->flush(f);
    }

    return Some(total);
}

}
