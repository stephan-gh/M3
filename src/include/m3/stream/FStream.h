/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
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
#include <base/stream/IStream.h>
#include <base/stream/OStream.h>

#include <m3/Exception.h>
#include <m3/tiles/OwnActivity.h>
#include <m3/vfs/File.h>
#include <m3/vfs/FileTable.h>

namespace m3 {

/**
 * FStream is an input- and output-stream for files. It uses m3::File as a backend and adds
 * buffering for the input and output.
 *
 * Note that if the file is in non-blocking mode, a "would block" return puts the FStream into an
 * error state. Therefore, clear_state() should be called before retrying the operation.
 */
class FStream : public IStream, public OStream {
    static const uint FL_DEL_BUF = 1;
    static const uint FL_DEL_FILE = 2;

    static int get_perms(int perms) {
        // if we want to write, we need read-permission to handle unaligned writes
        if((perms & FILE_RW) == FILE_W)
            return perms | FILE_R;
        return perms;
    }

public:
    static const uint FL_LINE_BUF = 4;

    /**
     * Binds this object to the given file descriptor and uses a buffer size of <bufsize>.
     *
     * @param fd the file descriptor
     * @param perms the permissions that determine which buffer to create (FILE_*)
     * @param bufsize the size of the buffer for input/output
     * @param flags the flags (FL_*)
     */
    explicit FStream(fd_t fd, int perms = FILE_RW, size_t bufsize = 512, uint flags = 0);

    /**
     * Opens <filename> with given permissions and a buffer size of <bufsize>. Which buffer is
     * created depends on <perms>.
     *
     * @param filename the file to open
     * @param perms the permissions (FILE_*)
     * @param bufsize the size of the buffer for input/output
     */
    explicit FStream(const char *filename, int perms = FILE_RW, size_t bufsize = 512);

    /**
     * Opens <filename> with given permissions and given buffer sizes.
     *
     * @param filename the file to open
     * @param rsize the size of the input-buffer (may be 0 if FILE_R is not set)
     * @param wsize the size of the output-buffer (may be 0 if FILE_W is not set)
     * @param perms the permissions (FILE_*)
     */
    explicit FStream(const char *filename, size_t rsize, size_t wsize, int perms = FILE_RW);

    virtual ~FStream();

    /**
     * @return the File instance
     */
    File *file() {
        return Activity::own().files()->get(_fd);
    }
    const File *file() const {
        return const_cast<FStream *>(this)->file();
    }

    /**
     * Retrieves information about this file
     *
     * @param info the struct to fill
     */
    void stat(FileInfo &info) const {
        if(bad())
            throw Exception(Errors::INV_STATE);

        file()->stat(info);
    }

    /**
     * Seeks to the given position.
     *
     * @param offset the offset to seek to (meaning depends on <whence>)
     * @param whence the seek type (M3FS_SEEK_*)
     * @return the new position
     */
    size_t seek(size_t offset, int whence);

    /**
     * Reads <count> bytes into <dst>. If the buffer is empty, the buffer is not used but it the
     * File instance is used directly.
     *
     * @param dst the destination to read into
     * @param count the number of bytes to read
     * @return the number of read bytes (or std::nullopt if it would block and we are in
     *     non-blocking mode)
     */
    std::optional<size_t> read(void *dst, size_t count);

    /**
     * Writes at most <count> bytes from <src> into the file. If the buffer is empty, the buffer is
     * not used but it the File instance is used directly.
     *
     * @param src the data to write
     * @param count the number of bytes to write
     * @return the number of written bytes (or std::nullopt if it would block and we are in
     *     non-blocking mode)
     */
    std::optional<size_t> write(const void *src, size_t count);

    /**
     * Writes all <count> bytes from <src> into the file.
     *
     * Note that this method works implicitly in blocking mode, regardless of the set mode.
     *
     * @param src the data to write
     * @param count the number of bytes to write
     * @return true if all bytes were written
     */
    bool write_all(const void *src, size_t count) {
        auto f = file();
        auto old_blocking = f->is_blocking();
        f->set_blocking(true);

        try {
            const uint8_t *s = static_cast<const uint8_t *>(src);
            while(!bad() && count) {
                size_t amount = write(s, count).value();
                count -= static_cast<size_t>(amount);
                s += amount;
            }
        }
        catch(...) {
            f->set_blocking(old_blocking);
            throw;
        }

        f->set_blocking(old_blocking);
        return count == 0;
    }

    /**
     * Flushes the internal write buffer
     */
    void flush();

    virtual char read() override {
        char c = '\0';
        read(&c, 1);
        return c;
    }
    virtual bool putback(char c) override {
        return _rbuf->putback(c);
    }
    virtual void write(char c) override {
        write(&c, 1);
    }

private:
    void set_error(std::optional<size_t> res);

    fd_t _fd;
    std::unique_ptr<File::Buffer> _rbuf;
    std::unique_ptr<File::Buffer> _wbuf;
    uint _flags;
};

}
