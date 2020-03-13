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

#pragma once

#include <base/Common.h>
#include <base/util/Reference.h>
#include <base/Errors.h>
#include <base/TCU.h>

#include <m3/com/EP.h>

#include <assert.h>

namespace m3 {

class File;
class GenericFile;
class VPE;

/**
 * The file descriptor table.
 *
 * The file table itself does not create or delete files. Instead, it only works with
 * pointers. The creation and deletion is done in VFS. The rational is, that VFS is used to
 * work with files, while FileTable is used to prepare the files for created VPEs. Thus, one
 * can simply add a file or remove a file from VPE::self() to a different VPE by passing a pointer
 * around. If the file table of a child VPE is completely setup, it is serialized and transferred
 * to the child VPE.
 */
class FileTable {
    friend class GenericFile;

public:
    static const fd_t MAX_FDS       = 64;

    /**
     * Constructor
     */
    explicit FileTable() noexcept
        : _fds() {
    }

    explicit FileTable(const FileTable &f) noexcept {
        for(fd_t i = 0; i < MAX_FDS; ++i)
            _fds[i] = f._fds[i];
    }
    FileTable &operator=(const FileTable &f) noexcept {
        if(&f != this) {
            for(fd_t i = 0; i < MAX_FDS; ++i)
                _fds[i] = f._fds[i];
        }
        return *this;
    }

    /**
     * Allocates a new file descriptor for given file.
     *
     * @param file the file
     * @return the file descriptor
     */
    fd_t alloc(Reference<File> file);

    /**
     * Removes and closes the given file descriptor
     *
     * @param fd the file descriptor
     */
    void remove(fd_t fd) noexcept;

    /**
     * @param fd the file descriptor
     * @return true if the given file descriptor exists
     */
    bool exists(fd_t fd) const noexcept {
        return _fds[fd];
    }

    /**
     * @param fd the file descriptor
     * @return the file for given fd
     */
    Reference<File> get(fd_t fd) const {
        // TODO throw if not existing?
        return _fds[fd];
    }

    /**
     * Sets <fd> to <file>.
     *
     * @param fd the file descriptor
     * @param file the file
     */
    void set(fd_t fd, Reference<File> file) {
        // TODO throw if existing?
        assert(file);
        _fds[fd] = file;
    }

    /**
     * Remove all files
     */
    void remove_all() noexcept;

    /**
     * Delegates all files to <vpe>.
     *
     * @param vpe the VPE to delegate the files to
     */
    void delegate(VPE &vpe) const;

    /**
     * Serializes the current files into the given buffer
     *
     * @param buffer the buffer
     * @param size the capacity of the buffer
     * @return the space used
     */
    size_t serialize(void *buffer, size_t size) const;

    /**
     * Unserializes the given buffer into a new FileTable object.
     *
     * @param buffer the buffer
     * @param size the size of the buffer
     * @return the FileTable object
     */
    static FileTable *unserialize(const void *buffer, size_t size);

private:
    EP get_ep();
    EP request_ep(GenericFile *file);

    Reference<File> _fds[MAX_FDS];
};

}
