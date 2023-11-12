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
#include <base/stream/OStringStream.h>
#include <base/util/Math.h>

#include <m3/com/MemGate.h>
#include <m3/com/SendGate.h>
#include <m3/tiles/Activity.h>

namespace m3 {

/**
 * A uni-directional pipe between two activities. An object of this class holds the state of the
 * pipe, i.e. the memory capability and the gate capability for communication. That means that the
 * object should stay alive as long as the pipe communication takes place.
 *
 * To use the pipe, this class creates two file descriptors for the read-end and write-end. After
 * being done with reading/writing, you need to close the file descriptor to notify the other
 * end. This is also required for the part that you do not use.
 *
 * Caution: the current implementation does only support the communication between the two
 * activities specified on construction.
 *
 * A usage example looks like the following:
 * <code>
 *   ChildActivity reader("reader");
 *
 *   // construct the pipe for activity::self -> reader
 *   Pipe pipe(reader, Activity::own(), 0x1000);
 *
 *   // bind the read-end to stdin of the child
 *   reader.add_file(STDIN_FD, pipe.reader_fd());
 *
 *   reader.run([] {
 *       // read from cin
 *       return 0;
 *   });
 *
 *   // we are done with reading
 *   pipe.close_reader();
 *
 *   File *out = Activity::own().files()->get(pipe.writer_fd());
 *   // write into out
 *
 *   // we are done with writing
 *   pipe.close_writer();
 *
 *   // wait until the reader exists before destroying the pipe
 *   reader.wait();
 * </code>
 */
class DirectPipe {
public:
    static const size_t MSG_SIZE = 64;
    static const size_t MSG_BUF_SIZE = MSG_SIZE * 16;
    static const size_t CREDITS = 16;

    enum {
        READ_EOF = 1 << 0,
        WRITE_EOF = 1 << 1,
    };

    /**
     * Creates a pipe with activity <rd> as the reader and <wr> as the writer, using a shared memory
     * area of <size> bytes.
     *
     * @param rd the reader of the pipe
     * @param wr the writer of the pipe
     * @param mem the shared memory area
     * @param size the size of the shared memory area
     */
    explicit DirectPipe(Activity &rd, Activity &wr, MemCap &mem, size_t size);
    DirectPipe(const DirectPipe &) = delete;
    DirectPipe &operator=(const DirectPipe &) = delete;
    ~DirectPipe();

    /**
     * @return the capabilities (rgate, memory and sgate)
     */
    capsel_t caps() const noexcept {
        return _rcap.sel();
    }
    /**
     * @return the size of the shared memory area
     */
    size_t size() const noexcept {
        return _size;
    }

    /**
     * @return the file descriptor for the reader
     */
    fd_t reader_fd() const noexcept {
        return _rdfd;
    }
    /**
     * Closes the read-end
     */
    void close_reader();

    /**
     * @return the file descriptor for the writer
     */
    fd_t writer_fd() const noexcept {
        return _wrfd;
    }
    /**
     * Closes the write-end
     */
    void close_writer();

private:
    Activity &_rd;
    Activity &_wr;
    size_t _size;
    RecvCap _rcap;
    MemCap _rmem;
    MemCap _wmem;
    SendCap _scap;
    fd_t _rdfd;
    fd_t _wrfd;
};

}
