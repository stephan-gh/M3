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

Option<size_t> File::Buffer::read(File *file, void *dst, size_t amount) {
    if(pos < cur) {
        size_t count = Math::min(amount, cur - pos);
        memcpy(dst, buffer.get() + pos, count);
        pos += count;
        return Some(count);
    }

    if(auto res = file->read(buffer.get(), size)) {
        size_t read = res.unwrap();
        if(read == 0)
            return Some(read);
        cur = read;

        size_t copyamnt = Math::min(cur, amount);
        memcpy(dst, buffer.get(), copyamnt);
        pos = copyamnt;
        return Some(copyamnt);
    }

    return None;
}

Option<size_t> File::Buffer::write(File *file, const void *src, size_t amount) {
    if(cur == size) {
        auto res = flush(file);
        // on errors or incomplete flushes, return error (Some(0)) or would block (None)
        if(res.is_none())
            return None;
        if(!res.unwrap())
            return Some(size_t(0));
    }

    size_t count = Math::min(size - cur, amount);
    memcpy(buffer.get() + cur, src, count);
    cur += count;
    return Some(count);
}

Option<bool> File::Buffer::flush(File *file) {
    if(auto write_res = file->write_all(buffer.get() + pos, cur - pos)) {
        pos += write_res.unwrap();
        if(pos == cur) {
            cur = 0;
            pos = 0;
            return Some(true);
        }
        return Some(false);
    }
    return None;
}

Option<size_t> File::write_all(const void *buffer, size_t count) {
    size_t total = count;
    const char *buf = reinterpret_cast<const char *>(buffer);
    while(count > 0) {
        auto write_res = write(buf, count);
        if(write_res.is_none() && total == 0)
            return None;

        size_t written = write_res.unwrap_or(0);
        if(written == 0)
            return Some(total - count);
        count -= written;
        buf += written;
    }
    return Some(total);
}

}
