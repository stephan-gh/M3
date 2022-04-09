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

#pragma once

#include <base/Common.h>
#include <base/Errors.h>
#include <base/TCU.h>

#include <m3/com/EP.h>
#include <m3/vfs/FileRef.h>
#include <m3/Exception.h>

#include <memory>
#include <assert.h>

namespace m3 {

class File;
class GenericFile;
class ChildActivity;

/**
 * The file descriptor table.
 *
 * The file table itself does not create or delete files. Instead, it only works with
 * pointers. The creation and deletion is done in VFS. The rational is, that VFS is used to
 * work with files, while FileTable is used to prepare the files for created activities. Thus, one
 * can simply add a file or remove a file from activity::self() to a different activity by passing a pointer
 * around. If the file table of a child activity is completely setup, it is serialized and transferred
 * to the child activity.
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

    explicit FileTable(const FileTable &f) = delete;
    FileTable &operator=(const FileTable &f) = delete;

    /**
     * Allocates a new file descriptor for given file.
     *
     * @param file the file
     * @return a FileRef to the file
     */
    template<class T>
    FileRef<T> alloc(std::unique_ptr<T> file) {
        return FileRef<T>(static_cast<T*>(do_alloc(std::move(file))));
    }

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
    File *get(fd_t fd) const {
        if(fd >= MAX_FDS || !_fds[fd])
            throw Exception(Errors::BAD_FD);
        return _fds[fd];
    }

    /**
     * Sets <fd> to <file>.
     *
     * @param fd the file descriptor
     * @param file the file
     */
    template<class T>
    void set(fd_t fd, FileRef<T> file) {
        do_set(fd, file.release());
    }

    /**
     * Remove all files
     */
    void remove_all() noexcept;

    /**
     * Delegates the files selected for the given activity to this activity.
     *
     * @param act the activity to delegate the files to
     */
    void delegate(ChildActivity &act) const;

    /**
     * Serializes the files of the given child activity into the given buffer
     *
     * @param act the child activity that should receive the files
     * @param buffer the buffer
     * @param size the capacity of the buffer
     * @return the space used
     */
    size_t serialize(ChildActivity &act, void *buffer, size_t size) const;

    /**
     * Unserializes the given buffer into a new FileTable object.
     *
     * @param buffer the buffer
     * @param size the size of the buffer
     * @return the FileTable object
     */
    static FileTable *unserialize(const void *buffer, size_t size);

private:
    File *do_alloc(std::unique_ptr<File> file);
    void do_set(fd_t fd, File *file);

    EP get_ep();
    EP request_ep(GenericFile *file);

    File *_fds[MAX_FDS];
};

}
