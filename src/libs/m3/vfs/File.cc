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

std::optional<size_t> File::Buffer::read(File *file, void *dst, size_t amount) {
    if(pos < cur) {
        size_t count = Math::min(amount, cur - pos);
        memcpy(dst, buffer.get() + pos, count);
        pos += count;
        return count;
    }

    if(auto res = file->read(buffer.get(), size)) {
        size_t read = res.value();
        if(read == 0)
            return 0;
        cur = read;

        size_t copyamnt = Math::min(cur, amount);
        memcpy(dst, buffer.get(), copyamnt);
        pos = copyamnt;
        return copyamnt;
    }

    return std::nullopt;
}

std::optional<size_t> File::Buffer::write(File *file, const void *src, size_t amount) {
    if(cur == size) {
        auto res = flush(file);
        // on errors or incomplete flushes, return error (0) or would block (std::nullopt)
        if(!res.has_value())
            return std::nullopt;
        if(!res.value())
            return 0;
    }

    size_t count = Math::min(size - cur, amount);
    memcpy(buffer.get() + cur, src, count);
    cur += count;
    return count;
}

std::optional<bool> File::Buffer::flush(File *file) {
    if(auto write_res = file->write_all(buffer.get() + pos, cur - pos)) {
        pos += write_res.value();
        if(pos == cur) {
            cur = 0;
            pos = 0;
            return true;
        }
        return false;
    }
    return std::nullopt;
}

std::optional<size_t> File::write_all(const void *buffer, size_t count) {
    size_t total = count;
    const char *buf = reinterpret_cast<const char *>(buffer);
    while(count > 0) {
        auto write_res = write(buf, count);
        if(!write_res.has_value() && total == 0)
            return std::nullopt;

        size_t written = write_res.value_or(0);
        if(written == 0)
            return total - count;
        count -= written;
        buf += written;
    }
    return total;
}

}
