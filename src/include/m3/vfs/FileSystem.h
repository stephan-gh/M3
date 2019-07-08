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

#include <base/util/Reference.h>
#include <base/util/String.h>
#include <base/Errors.h>

#include <fs/internal.h>

namespace m3 {

class File;
class Marshaller;

/**
 * The base-class of all filesystems
 */
class FileSystem : public RefCounted {
public:
    explicit FileSystem() noexcept {
    }
    virtual ~FileSystem() {
    }

    /**
     * @return for serialization: the type of fs
     */
    virtual char type() const noexcept = 0;

    /**
     * Creates a File-instance from given path with given permissions.
     *
     * @param path the filepath
     * @param perms the permissions (FILE_*)
     * @return the File-instance
     */
    virtual Reference<File> open(const char *path, int perms) = 0;

    /**
     * Retrieves the file information for the given path.
     *
     * @param path the path
     * @param info where to write to
     */
    virtual void stat(const char *path, FileInfo &info) = 0;

    /**
     * Creates the given directory.
     *
     * @param path the directory path
     * @param mode the permissions to assign
     */
    virtual void mkdir(const char *path, mode_t mode) = 0;

    /**
     * Removes the given directory. It needs to be empty.
     *
     * @param path the directory path
     */
    virtual void rmdir(const char *path) = 0;

    /**
     * Creates a link at <newpath> to <oldpath>.
     *
     * @param oldpath the existing path
     * @param newpath tne link to create
     */
    virtual void link(const char *oldpath, const char *newpath) = 0;

    /**
     * Removes the given file.
     *
     * @param path the path
     */
    virtual void unlink(const char *path) = 0;

    /**
     * Delegates all this filesystem to the given VPE.
     *
     * @param vpe the VPE
     */
    virtual void delegate(VPE &vpe) = 0;

    /**
     * Serializes this object to the given marshaller.
     *
     * @param m the marshaller
     */
    virtual void serialize(Marshaller &m) = 0;

    /**
     * Delegates the given EP caps to the server.
     *
     * @param first the first EP cap
     * @param count the number of caps
     */
    virtual void delegate_eps(capsel_t first, uint count) = 0;
};

}
