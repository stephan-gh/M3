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

#include <base/Common.h>
#include <base/util/Reference.h>

#include <m3/com/Marshalling.h>

#include <fs/internal.h>

#include <memory>

namespace m3 {

class VFS;
class FStream;
class FileTable;
class Pager;

/**
 * The base-class of all files. Can't be instantiated.
 */
class File : public RefCounted {
    friend class FStream;
    friend class FileTable;

protected:
    explicit File() {
    }

public:
    /**
     * The default buffer implementation
     */
    struct Buffer {
        /**
         * Creates a buffer with <_size> bytes.
         *
         * @param _size the number of bytes (0 = no buffer)
         */
        explicit Buffer(size_t _size)
            : buffer(_size ? new char[_size] : nullptr),
              size(_size),
              cur(),
              pos() {
        }

        /**
         * @return true if the buffer is empty
         */
        bool empty() noexcept {
            return cur == 0;
        }
        /**
         * Invalidates the buffer, i.e. makes it empty
         */
        void invalidate() noexcept {
            cur = 0;
        }

        /**
         * Puts the given character back into the buffer.
         *
         * @param c the character
         * @return true if successful
         */
        bool putback(char c);

        /**
         * Reads <amount> bytes from the buffer into <dst>.
         *
         * @param file the file backend
         * @param dst the destination buffer
         * @param amount the number of bytes to read
         * @return the number of read bytes (0 = EOF)
         */
        size_t read(File *file, void *dst, size_t amount);

        /**
         * Writes <amount> bytes from <src> into the buffer.
         *
         * @param file the file backend
         * @param src the data to write
         * @param amount the number of bytes to write
         * @return the number of written bytes (0 = EOF)
         */
        size_t write(File *file, const void *src, size_t amount);

        /**
         * Flushes the buffer.
         *
         * @param file the file backend
         */
        void flush(File *file);

        std::unique_ptr<char[]> buffer;
        size_t size;
        size_t cur;
        size_t pos;
    };

    explicit File(int flags) noexcept
        : _flags(flags),
          _fd() {
    }
    File(const File &) = delete;
    File &operator=(const File &) = delete;
    virtual ~File() {
    }

    /**
     * @return the open flags
     */
    int flags() const noexcept {
        return _flags;
    }

    /**
     * @return the file descriptor
     */
    fd_t fd() const noexcept {
        return _fd;
    }

    /**
     * Retrieves information about this file
     *
     * @param info the struct to fill
     */
    void stat(FileInfo &info) const {
        Errors::Code res = try_stat(info);
        if(res != Errors::NONE)
            throw Exception(res);
    }

    /**
     * Tries to retrieve information about this file. That is, on error it does not throw an
     * exception, but returns the error code.
     *
     * @param info the struct to fill
     * @return the error on failure
     */
    virtual Errors::Code try_stat(FileInfo &info) const = 0;

    /**
     * Changes the file-position to <offset>, using <whence>.
     *
     * @param offset the offset to use
     * @param whence the seek-type (M3FS_SEEK_{SET,CUR,END}).
     * @return the new file-position
     */
    virtual size_t seek(size_t offset, int whence) = 0;

    /**
     * Reads at most <count> bytes into <buffer>.
     *
     * @param buffer the buffer to read into
     * @param count the number of bytes to read
     * @return the number of read bytes
     */
    virtual size_t read(void *buffer, size_t count) = 0;

    /**
     * Writes at most <count> bytes from <buffer> into the file.
     *
     * @param buffer the data to write
     * @param count the number of bytes to write
     * @return the number of written bytes
     */
    virtual size_t write(const void *buffer, size_t count) = 0;

    /**
     * Writes <count> bytes from <buffer> into the file, if possible.
     *
     * @param buffer the data to write
     * @param count the number of bytes to write
     */
    void write_all(const void *buffer, size_t count) {
        const char *buf = reinterpret_cast<const char*>(buffer);
        while(count > 0) {
            size_t res = write(buf, count);
            count -= static_cast<size_t>(res);
            buf += static_cast<size_t>(res);
        }
    }

    /**
     * Flush the locally written data to the file system.
     */
    virtual void flush() {
    }

    /**
     * Ensure that the file is made persistent.
     */
    virtual void sync() {
    }

    /**
     * Maps the range <fileoff>..<fileoff>+<len> to *<virt> with given flags.
     *
     * @param pager the pager to use
     * @param virt the virtual address (0 = automatic); will be set to the chosen address
     * @param fileoff the file offset to start the mapping at
     * @param len the number of bytes to map
     * @param prot the protection flags (see Pager::Prot::*)
     * @param flags the mapping flags (see Pager::Flags::*)
     */
    virtual void map(Reference<Pager> &pager, goff_t *virt, size_t fileoff, size_t len,
                     int prot, int flags) const = 0;

    /**
     * @return the unique character for serialization
     */
    virtual char type() const noexcept = 0;

    /**
     * Obtains a new file session from the server
     *
     * @return the new file
     */
    virtual Reference<File> clone() const = 0;

    /**
     * Delegates this file to the given VPE.
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

private:
    virtual void close() noexcept = 0;

    void set_fd(fd_t fd) noexcept {
        _fd = fd;
    }

    int _flags;
    fd_t _fd;
};

}
