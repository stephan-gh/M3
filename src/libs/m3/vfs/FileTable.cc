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

#include <base/log/Lib.h>
#include <base/Panic.h>

#include <m3/com/Marshalling.h>
#include <m3/pipe/DirectPipeReader.h>
#include <m3/pipe/DirectPipeWriter.h>
#include <m3/vfs/FileTable.h>
#include <m3/vfs/File.h>
#include <m3/vfs/GenericFile.h>
#include <m3/vfs/SerialFile.h>

namespace m3 {

void FileTable::remove_all() noexcept {
    for(fd_t i = 0; i < FileTable::MAX_FDS; ++i)
        Activity::self().files()->remove(i);
}

fd_t FileTable::alloc(Reference<File> file) {
    for(fd_t i = 0; i < MAX_FDS; ++i) {
        if(!_fds[i]) {
            LLOG(FILES, "FileTable[" << i << "] = file");
            file->set_fd(i);
            _fds[i] = file;
            return i;
        }
    }

    throw MessageException("No free file descriptor", Errors::NO_SPACE);
}

void FileTable::remove(fd_t fd) noexcept {
    Reference<File> file = _fds[fd];

    if(file) {
        // close the file (important for, e.g., pipes)
        file->remove();

        // remove from file table
        _fds[fd].unref();

        LLOG(FILES, "FileTable[" << fd << "] = --");
    }
}

void FileTable::delegate(Activity &act) const {
    for(fd_t i = 0; i < MAX_FDS; ++i) {
        if(_fds[i]) {
            LLOG(FILES, "FileTable[" << i << "] = delegate");
            _fds[i]->delegate(act);
        }
    }
}

size_t FileTable::serialize(void *buffer, size_t size) const {
    Marshaller m(static_cast<unsigned char*>(buffer), size);

    size_t count = 0;
    for(fd_t i = 0; i < MAX_FDS; ++i) {
        if(_fds[i])
            count++;
    }

    m << count;
    for(fd_t i = 0; i < MAX_FDS; ++i) {
        if(_fds[i]) {
            m << i << _fds[i]->type();
            _fds[i]->serialize(m);
        }
    }
    return m.total();
}

FileTable *FileTable::unserialize(const void *buffer, size_t size) {
    FileTable *obj = new FileTable();
    Unmarshaller um(static_cast<const unsigned char*>(buffer), size);
    size_t count;
    um >> count;
    while(count-- > 0) {
        fd_t fd;
        char type;
        um >> fd >> type;
        switch(type) {
            case 'F':
                obj->_fds[fd] = Reference<File>(GenericFile::unserialize(um));
                break;
            case 'S':
                obj->_fds[fd] = Reference<File>(SerialFile::unserialize(um));
                break;
            case 'P':
                obj->_fds[fd] = Reference<File>(DirectPipeWriter::unserialize(um));
                break;
            case 'Q':
                obj->_fds[fd] = Reference<File>(DirectPipeReader::unserialize(um));
                break;
        }
    }
    return obj;
}

}
