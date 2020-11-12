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

#include <base/log/Lib.h>
#include <base/stream/Serial.h>
#include <base/Init.h>

#include <m3/com/Marshalling.h>
#include <m3/vfs/File.h>
#include <m3/vfs/FileTable.h>
#include <m3/vfs/MountTable.h>
#include <m3/vfs/VFS.h>
#include <m3/pes/VPE.h>

namespace m3 {

// clean them up after the standard streams have been destructed
INIT_PRIO_VFS VFS::Cleanup VFS::_cleanup;

VFS::Cleanup::~Cleanup() {
    VPE::self().fds()->remove_all();
    VPE::self().mounts()->remove_all();
}

std::unique_ptr<MountTable> &VFS::ms() {
    return VPE::self().mounts();
}

void VFS::mount(const char *path, const char *fs, const char *options) {
    if(ms()->indexof_mount(path) != MountTable::MAX_MOUNTS)
        throw Exception(Errors::EXISTS);

    FileSystem *fsobj;
    if(strcmp(fs, "m3fs") == 0)
        fsobj = new M3FS(options ? options : fs);
    else
        VTHROW(Errors::INV_ARGS, "Unknown filesystem '" << fs << "'");
    ms()->add(path, fsobj);
}

void VFS::unmount(const char *path) {
    ms()->remove(path);
}

fd_t VFS::open(const char *path, int flags) {
    try {
        size_t pos;
        Reference<FileSystem> fs = ms()->resolve(path, &pos);
        Reference<File> file = fs->open(path + pos, flags);
        fd_t fd = VPE::self().fds()->alloc(file);
        LLOG(FS, "GenFile[" << fd << "]::open(" << path << ", " << flags << ")");
        if(flags & FILE_APPEND)
            file->seek(0, M3FS_SEEK_END);
        return fd;
    }
    catch(const Exception &e) {
        VTHROW(e.code(), "Unable to open '" << path << "' with flags=" << flags);
    }
}

void VFS::close(fd_t fd) noexcept {
    VPE::self().fds()->remove(fd);
}

void VFS::stat(const char *path, FileInfo &info) {
    try {
        size_t pos;
        Reference<FileSystem> fs = ms()->resolve(path, &pos);
        fs->stat(path + pos, info);
    }
    catch(const Exception &e) {
        VTHROW(e.code(), "stat '" << path << "' failed");
    }
}

Errors::Code VFS::try_stat(const char *path, FileInfo &info) noexcept {
    size_t pos;
    Reference<FileSystem> fs = ms()->try_resolve(path, &pos);
    if(!fs)
        return Errors::NO_SUCH_FILE;
    return fs->try_stat(path + pos, info);
}

void VFS::mkdir(const char *path, mode_t mode) {
    try {
        size_t pos;
        Reference<FileSystem> fs = ms()->resolve(path, &pos);
        return fs->mkdir(path + pos, mode);
    }
    catch(const Exception &e) {
        VTHROW(e.code(), "mkdir '" << path << "' failed");
    }
}

void VFS::rmdir(const char *path) {
    try {
        size_t pos;
        Reference<FileSystem> fs = ms()->resolve(path, &pos);
        return fs->rmdir(path + pos);
    }
    catch(const Exception &e) {
        VTHROW(e.code(), "rmdir '" << path << "' failed");
    }
}

void VFS::link(const char *oldpath, const char *newpath) {
    try {
        size_t pos1, pos2;
        Reference<FileSystem> fs1 = ms()->resolve(oldpath, &pos1);
        Reference<FileSystem> fs2 = ms()->resolve(newpath, &pos2);
        if(fs1.get() != fs2.get())
            throw Exception(Errors::XFS_LINK);
        return fs1->link(oldpath + pos1, newpath + pos2);
    }
    catch(const Exception &e) {
        VTHROW(e.code(), "link '" << oldpath << "' to '" << newpath << "' failed");
    }
}

void VFS::unlink(const char *path) {
    try {
        size_t pos;
        Reference<FileSystem> fs = ms()->resolve(path, &pos);
        return fs->unlink(path + pos);
    }
    catch(const Exception &e) {
        VTHROW(e.code(), "unlink '" << path << "' failed");
    }
}

void VFS::rename(const char *oldpath, const char *newpath) {
    try {
        size_t pos1, pos2;
        Reference<FileSystem> fs1 = ms()->resolve(oldpath, &pos1);
        Reference<FileSystem> fs2 = ms()->resolve(newpath, &pos2);
        if(fs1.get() != fs2.get())
            throw Exception(Errors::XFS_LINK);
        return fs1->rename(oldpath + pos1, newpath + pos2);
    }
    catch(const Exception &e) {
        VTHROW(e.code(), "rename '" << oldpath << "' to '" << newpath << "' failed");
    }
}

void VFS::print(OStream &os) noexcept {
    VPE::self().mounts()->print(os);
}

}
