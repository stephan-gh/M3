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

namespace m3 {

class File;

extern void close_file(File *file);

/**
 * Holds a reference to a file.
 *
 * This class gives direct access to a concrete file implementation and closes the file
 * automatically on destruction.
 */
template<class T>
class FileRef {
    template<class U>
    friend class FileRef;

public:
    /**
     * Creates a new file reference for given file.
     *
     * @param file the file
     */
    explicit FileRef(T *file = nullptr) : _file(file) {
    }
    template<class U>
    FileRef(FileRef<U> &&f) noexcept : _file(static_cast<T*>(f._file)) {
        f._file = nullptr;
    }
    template<class U>
    FileRef &operator=(FileRef<U> &&f) {
        _file = f._file;
        f._file = nullptr;
        return *this;
    }
    FileRef(const FileRef&) = delete;
    FileRef &operator=(const FileRef&) = delete;
    ~FileRef() {
        reset();
    }

    /**
     * Releases the file to the caller. That is, the file will not be closed on destruction of this
     * file reference anymore.
     *
     * @return the file
     */
    T *release() {
        auto file = _file;
        _file = nullptr;
        return file;
    }

    /**
     * Resets this file reference to the given file or no file. Note that the current file is
     * closed, if any is set.
     *
     * @param nfile the new file to bind this file reference to. By default, no file will be set,
     *     invalidating this file reference.
     */
    void reset(T *nfile = nullptr) {
        if(_file)
            close_file(_file);
        _file = nfile;
    }

    /**
     * @return true if this reference refers to a file
     */
    bool is_valid() const noexcept {
        return _file != nullptr;
    }

    T *operator->() noexcept {
        return _file;
    }
    const T *operator->() const noexcept {
        return _file;
    }
    T &operator*() noexcept {
        return *_file;
    }
    const T &operator*() const noexcept {
        return *_file;
    }

private:
    T *_file;
};

}
