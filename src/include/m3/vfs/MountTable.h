/*
 * Copyright (C) 2017-2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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
#include <base/util/String.h>
#include <base/Errors.h>

#include <m3/vfs/FileSystem.h>

namespace m3 {

class VPE;

/**
 * Contains a list of mount points and offers operations to manage them.
 *
 * The mount table itself does not create or delete mount points. Instead, it only works with
 * pointers. The creation and deletion is done in VFS. The rational is, that VFS is used to
 * manipulate the mounts of the own VPE, while MountTable is used to manipulate the mounts of
 * created VPEs. Thus, one can simply add a mointpoint from VPE::self() to a different VPE by
 * passing a pointer around. If the mount table of a child VPE is completely setup, it is serialized
 * and transferred to the child VPE.
 */
class MountTable {
    class MountPoint {
    public:
        explicit MountPoint(const char *path, FileSystem *fs) noexcept
            : _path(path),
              _fs(fs) {
        }

        const String &path() const noexcept {
            return _path;
        }
        const Reference<FileSystem> &fs() const noexcept {
            return _fs;
        }

    private:
        String _path;
        Reference<FileSystem> _fs;
    };

public:
    static const size_t MAX_MOUNTS  = 4;

    /**
     * Constructor
     */
    explicit MountTable() noexcept
        : _count(),
          _next_id(),
          _mounts() {
    }

    explicit MountTable(const MountTable &ms) noexcept;
    MountTable &operator=(const MountTable &ms) noexcept;

    /**
     * Allocates a new id for the next filesystem
     *
     * @return the next id
     */
    size_t alloc_id() noexcept {
        return _next_id++;
    }

    /**
     * Adds the given mountpoint
     *
     * @param path the path
     * @param fs the filesystem instance
     */
    void add(const char *path, FileSystem *fs);

    /**
     * Resolves the given path to a mounted filesystem.
     *
     * @param path the path
     * @param pos will be set to the position within the path where the mounted FS starts
     * @return the filesystem
     */
    Reference<FileSystem> resolve(const char *path, size_t *pos);

    /**
     * Tries to resolves the given path to a mounted filesystem. That is, on error, it does not
     * throw an exception, but returns an invalid reference.
     *
     * @param path the path
     * @param pos will be set to the position within the path where the mounted FS starts
     * @return the filesystem or an invalid reference
     */
    Reference<FileSystem> try_resolve(const char *path, size_t *pos) noexcept;

    /**
     * @param path the path
     * @return the index of the mountpoint at given path
     */
    size_t indexof_mount(const char *path);

    /**
     * Removes the mountpoint at given path.
     *
     * @param path the path
     */
    void remove(const char *path);

    /**
     * Removes all mountpoints.
     */
    void remove_all() noexcept;

    /**
     * Delegates the mount points to <vpe>.
     *
     * @param vpe the VPE to delegate the caps to
     */
    void delegate(VPE &vpe) const;

    /**
     * Serializes the current mounts into the given buffer
     *
     * @param buffer the buffer
     * @param size the capacity of the buffer
     * @return the space used
     */
    size_t serialize(void *buffer, size_t size) const;

    /**
     * Unserializes the mounts from the buffer into a new MountTable object.
     *
     * @param buffer the buffer
     * @param size the length of the data
     * @return the mount table
     */
    static MountTable *unserialize(const void *buffer, size_t size);

    /**
     * Prints the current mounts to <os>.
     *
     * @param os the stream to write to
     */
    void print(OStream &os) const noexcept;

private:
    void do_remove(size_t i);

    size_t _count;
    size_t _next_id;
    MountPoint *_mounts[MAX_MOUNTS];
};

}
