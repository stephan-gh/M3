/*
 * Copyright (C) 2016-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <m3/vfs/File.h>

namespace m3 {

bool File::Buffer::putback(char c) {
    if(cur > 0 && pos > 0) {
        buffer[--pos] = c;
        return true;
    }
    return false;
}

ssize_t File::Buffer::read(File *file, void *dst, size_t amount) {
    if(pos < cur) {
        size_t count = Math::min(amount, cur - pos);
        memcpy(dst, buffer.get() + pos, count);
        pos += count;
        return static_cast<ssize_t>(count);
    }

    ssize_t res = file->read(buffer.get(), size);
    if(res <= 0)
        return res;
    cur = static_cast<size_t>(res);

    size_t copyamnt = Math::min(static_cast<size_t>(res), amount);
    memcpy(dst, buffer.get(), copyamnt);
    pos = copyamnt;
    return static_cast<ssize_t>(copyamnt);
}

ssize_t File::Buffer::write(File *file, const void *src, size_t amount) {
    if(cur == size) {
        int res = flush(file);
        // on errors or incomplete flushes, return error (0) or would block (-1)
        if(res <= 0)
            return res;
    }

    size_t count = Math::min(size - cur, amount);
    memcpy(buffer.get() + cur, src, count);
    cur += count;
    return static_cast<ssize_t>(count);
}

int File::Buffer::flush(File *file) {
    ssize_t written = file->write_all(buffer.get() + pos, cur - pos);
    if(written == 0)
        return 0;
    if(written > 0)
        pos += static_cast<size_t>(written);
    if(pos == cur) {
        cur = 0;
        pos = 0;
        return 1;
    }
    return -1;
}

ssize_t File::write_all(const void *buffer, size_t count) {
    size_t total = count;
    const char *buf = reinterpret_cast<const char *>(buffer);
    while(count > 0) {
        ssize_t written = write(buf, count);
        if(written == -1 && total == count)
            return -1;
        if(written <= 0)
            return static_cast<ssize_t>(total - count);
        count -= static_cast<size_t>(written);
        buf += static_cast<size_t>(written);
    }
    return static_cast<ssize_t>(total);
}

}
