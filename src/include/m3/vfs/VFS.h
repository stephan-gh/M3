/*
 * Copyright (C) 2015-2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/col/SList.h>
#include <base/util/Reference.h>
#include <base/DTU.h>

#include <m3/session/M3FS.h>
#include <m3/vfs/File.h>
#include <m3/vfs/FileSystem.h>

namespace m3 {

/**
 * A VPE-local virtual file system. It allows to mount filesystems at a given path and directs
 * filesystem operations like open, mkdir, ... to the corresponding filesystem.
 */
class VFS {
    static const size_t MAX_RES_EPS = 2;

    struct Cleanup {
        Cleanup() {
        }
        ~Cleanup();
    };

    class ReservedEPs {
        friend class VFS;

    public:
        explicit ReservedEPs()
            : _fs(),
              _eps(),
              _eps_count(),
              _eps_used() {
        }
        explicit ReservedEPs(Reference<FileSystem> fs, capsel_t first, uint count)
            : _fs(fs),
              _eps(first),
              _eps_count(count),
              _eps_used() {
        }

        bool has_ep(capsel_t ep) const {
            return ep >= _eps && ep < _eps + _eps_count;
        }
        capsel_t alloc_ep() {
            for(uint i = 0; i < _eps_count; ++i) {
                if((_eps_used & (1u << i)) == 0) {
                    _eps_used |= 1u << i;
                    return _eps + i;
                }
            }
            return ObjCap::INVALID;
        }

        void free_ep(capsel_t ep) {
            _eps_used &= ~(1u << (ep - _eps));
        }

    private:
        Reference<FileSystem> _fs;
        capsel_t _eps;
        uint _eps_count;
        uint _eps_used;
    };

public:
    /**
     * Mounts <fs> at given path
     *
     * @param path the path
     * @param fs the filesystem name
     * @param options options to pass to the filesystem
     * @return Errors::NONE on success
     */
    static Errors::Code mount(const char *path, const char *fs, const char *options = nullptr);

    /**
     * Unmounts the filesystem at <path>.
     *
     * @param path the path
     */
    static void unmount(const char *path);

    /**
     * Delegates the given EP caps to the file system at <path>.
     *
     * @param first the first EP cap
     * @param count the number of caps
     * @return the error, if any
     */
    static Errors::Code delegate_eps(const char *path, capsel_t first, uint count);

    /**
     * Allocates an EP for the given file system. Does only succeed if delegate_eps() was called
     * before for this file system.
     *
     * @param fs the file system
     * @param idx will be set to the index of the ep
     * @return the ep capability or ObjCap::INVALID
     */
    static capsel_t alloc_ep(const Reference<FileSystem> &fs, size_t *idx);

    /**
     * Free's the EP that has previously been allocated via alloc_ep().
     *
     * @param ep the ep capability
     */
    static void free_ep(capsel_t ep);

    /**
     * Opens the file at <path> using the given permissions.
     *
     * @param path the path to the file to open
     * @param perms the permissions (FILE_*)
     * @return the file descriptor or FileTable::INVALID if it failed
     */
    static fd_t open(const char *path, int perms);

    /**
     * Closes the given file
     *
     * @param fd the file descriptor
     */
    static void close(fd_t fd);

    /**
     * Retrieves the file information for the given path.
     *
     * @param path the path
     * @param info where to write to
     * @return the error, if any happened
     */
    static Errors::Code stat(const char *path, FileInfo &info);

    /**
     * Creates the given directory. Expects that all path-components except the last already exists.
     *
     * @param path the path
     * @param mode the permissions to assign
     * @return the error, if any happened
     */
    static Errors::Code mkdir(const char *path, mode_t mode);

    /**
     * Removes the given directory. It needs to be empty.
     *
     * @param path the path
     * @return the error, if any happened
     */
    static Errors::Code rmdir(const char *path);

    /**
     * Creates a link at <newpath> to <oldpath>.
     *
     * @param oldpath the existing path
     * @param newpath tne link to create
     * @return the error, if any happened
     */
    static Errors::Code link(const char *oldpath, const char *newpath);

    /**
     * Removes the given path.
     *
     * @param path the path
     * @return the error, if any happened
     */
    static Errors::Code unlink(const char *path);

    /**
     * Prints the current mounts to <os>.
     *
     * @param os the stream to write to
     */
    static void print(OStream &os);

private:
    static MountTable *ms();

    static Cleanup _cleanup;
    static ReservedEPs _reseps[MAX_RES_EPS];
};

}
