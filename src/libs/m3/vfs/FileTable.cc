/*
 * Copyright (C) 2016-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/Panic.h>
#include <base/log/Lib.h>

#include <m3/com/Marshalling.h>
#include <m3/pipe/DirectPipeReader.h>
#include <m3/pipe/DirectPipeWriter.h>
#include <m3/vfs/File.h>
#include <m3/vfs/FileTable.h>
#include <m3/vfs/GenericFile.h>
#include <m3/vfs/SerialFile.h>

namespace m3 {

void FileTable::remove_all() noexcept {
    for(fd_t i = 0; i < FileTable::MAX_FDS; ++i)
        Activity::own().files()->remove(i);
}

File *FileTable::do_alloc(std::unique_ptr<File> file) {
    for(fd_t i = 0; i < MAX_FDS; ++i) {
        if(!_fds[i]) {
            LLOG(FILES, "FileTable[{}] = file"_cf, i);
            file->set_fd(i);
            _fds[i] = file.release();
            return _fds[i];
        }
    }

    throw MessageException("No free file descriptor", Errors::NO_SPACE);
}

void FileTable::do_set(fd_t fd, File *file) {
    if(file->fd() == fd)
        return;

    if(_fds[fd])
        remove(fd);
    if(file->fd() != -1)
        _fds[file->fd()] = nullptr;
    file->set_fd(fd);
    _fds[fd] = file;
}

void FileTable::remove(fd_t fd) noexcept {
    if(_fds[fd]) {
        // close the file (important for, e.g., pipes)
        _fds[fd]->remove();

        // remove from file table
        delete _fds[fd];
        _fds[fd] = nullptr;

        LLOG(FILES, "FileTable[{}] = --"_cf, fd);
    }
}

void FileTable::delegate(ChildActivity &act) const {
    for(auto mapping = act._files.begin(); mapping != act._files.end(); ++mapping) {
        auto file = Activity::own().files()->get(mapping->second);
        LLOG(FILES, "FileTable[{}] = delegate"_cf, mapping->second);
        file->delegate(act);
    }
}

size_t FileTable::serialize(ChildActivity &act, void *buffer, size_t size) const {
    Marshaller m(static_cast<unsigned char *>(buffer), size);

    size_t count = act._files.size();
    m << count;
    for(auto mapping = act._files.begin(); mapping != act._files.end(); ++mapping) {
        auto file = Activity::own().files()->get(mapping->second);
        m << mapping->first << file->type();
        file->serialize(m);
    }
    return m.total();
}

FileTable *FileTable::unserialize(const void *buffer, size_t size) {
    FileTable *obj = new FileTable();
    Unmarshaller um(static_cast<const unsigned char *>(buffer), size);
    size_t count;
    um >> count;
    while(count-- > 0) {
        fd_t fd;
        char type;
        um >> fd >> type;
        switch(type) {
            case 'F': obj->do_set(fd, GenericFile::unserialize(um)); break;
            case 'S': obj->do_set(fd, SerialFile::unserialize(um)); break;
            case 'P': obj->do_set(fd, DirectPipeWriter::unserialize(um)); break;
            case 'Q': obj->do_set(fd, DirectPipeReader::unserialize(um)); break;
        }
    }
    return obj;
}

}
