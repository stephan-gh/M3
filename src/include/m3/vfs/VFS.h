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
#include <base/TCU.h>

#include <m3/session/M3FS.h>
#include <m3/vfs/File.h>
#include <m3/vfs/FileSystem.h>
#include <m3/vfs/MountTable.h>

namespace m3 {

/**
 * A VPE-local virtual file system. It allows to mount filesystems at a given path and directs
 * filesystem operations like open, mkdir, ... to the corresponding filesystem.
 */
class VFS {
    struct Cleanup {
        Cleanup() {
        }
        ~Cleanup();
    };

public:
    /**
     * Mounts <fs> at given path
     *
     * @param path the path
     * @param fs the filesystem name
     * @param options options to pass to the filesystem
     */
    static void mount(const char *path, const char *fs, const char *options = nullptr);

    /**
     * Unmounts the filesystem at <path>.
     *
     * @param path the path
     */
    static void unmount(const char *path);

    /**
     * Opens the file at <path> using the given permissions.
     *
     * @param path the path to the file to open
     * @param perms the permissions (FILE_*)
     * @return the file descriptor
     */
    static fd_t open(const char *path, int perms);

    /**
     * Closes the given file
     *
     * @param fd the file descriptor
     */
    static void close(fd_t fd) noexcept;

    /**
     * Retrieves the file information for the given path.
     *
     * @param path the path
     * @param info where to write to
     */
    static void stat(const char *path, FileInfo &info);

    /**
     * Tries to retrieve the file information for the given path. That is, on error it does not
     * throw an exception, but returns the error code.
     *
     * @param path the path
     * @param info where to write to
     * @return the error code on failure
     */
    static Errors::Code try_stat(const char *path, FileInfo &info) noexcept;

    /**
     * Creates the given directory. Expects that all path-components except the last already exists.
     *
     * @param path the path
     * @param mode the permissions to assign
     */
    static void mkdir(const char *path, mode_t mode);

    /**
     * Tries to create the given directory. That is, on error it does not throw an exception, but
     * returns the error code. Expects that all path-components except the last already exists.
     *
     * @param path the path
     * @param mode the permissions to assign
     * @return the error code on failure
     */
    static Errors::Code try_mkdir(const char *path, mode_t mode);

    /**
     * Removes the given directory. It needs to be empty.
     *
     * @param path the path
     */
    static void rmdir(const char *path);

    /**
     * Tries to remove the given directory. That is, on error it does not throw an exception, but
     * returns the error code. It needs to be empty.
     *
     * @param path the path
     * @return the error code on failure
     */
    static Errors::Code try_rmdir(const char *path);

    /**
     * Creates a link at <newpath> to <oldpath>.
     *
     * @param oldpath the existing path
     * @param newpath the link to create
     */
    static void link(const char *oldpath, const char *newpath);

    /**
     * Tries to create a link at <newpath> to <oldpath>. That is, on error it does not throw an
     * exception, but returns the error code.
     *
     * @param oldpath the existing path
     * @param newpath the link to create
     * @return the error code on failure
     */
    static Errors::Code try_link(const char *oldpath, const char *newpath);

    /**
     * Removes the given path.
     *
     * @param path the path
     */
    static void unlink(const char *path);

    /**
     * Tries to remove the given path. That is, on error it does not throw an exception, but returns
     * the error code.
     *
     * @param path the path
     * @return the error code on failure
     */
    static Errors::Code try_unlink(const char *path);

    /**
     * Renames <oldpath> to <newpath>.
     *
     * @param oldpath the existing path
     * @param newpath the new path
     */
    static void rename(const char *oldpath, const char *newpath);

    /**
     * Tries to rename <oldpath> to <newpath>. That is, on error it does not throw an exception, but
     * returns the error code.
     *
     * @param oldpath the existing path
     * @param newpath the new path
     * @return the error code on failure
     */
    static Errors::Code try_rename(const char *oldpath, const char *newpath);

    /**
     * Prints the current mounts to <os>.
     *
     * @param os the stream to write to
     */
    static void print(OStream &os) noexcept;

private:
    static std::unique_ptr<MountTable> &ms();

    static Cleanup _cleanup;
};

}
