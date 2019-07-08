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

#include <m3/vfs/File.h>

namespace m3 {

bool File::Buffer::putback(char c) {
    if(cur > 0 && pos > 0) {
        buffer[--pos] = c;
        return true;
    }
    return false;
}

size_t File::Buffer::read(File *file, void *dst, size_t amount) {
    if(pos < cur) {
        size_t count = Math::min(amount, cur - pos);
        memcpy(dst, buffer.get() + pos, count);
        pos += count;
        return count;
    }

    size_t res = file->read(buffer.get(), size);
    if(res == 0)
        return 0;
    cur = res;

    size_t copyamnt = Math::min(static_cast<size_t>(res), amount);
    memcpy(dst, buffer.get(), copyamnt);
    pos = copyamnt;
    return copyamnt;
}

size_t File::Buffer::write(File *file, const void *src, size_t amount) {
    if(cur == size)
        flush(file);

    size_t count = Math::min(size - cur, amount);
    memcpy(buffer.get() + cur, src, count);
    cur += count;
    return count;
}

void File::Buffer::flush(File *file) {
    file->write_all(buffer.get(), cur);
    cur = 0;
}

}
