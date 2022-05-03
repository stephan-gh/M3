/*
 * Copyright (C) 2019-2020 Nils Asmussen, Barkhausen Institut
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

#include <base/Errors.h>
#include <base/KIF.h>
#include <base/stream/OStringStream.h>
#include <base/util/String.h>

/**
 * This macro throws an exception and passes a formatted string as its message. That is, you can
 * use the stream operators to build the message. For example:
 * VTHROW(EXISTS, "My exception " << 1 << "," << 2 << " message");
 */
#define VTHROW(error, expr)                            \
    {                                                  \
        m3::OStringStream __os;                        \
        __os << expr;                                  \
        throw m3::MessageException(__os.str(), error); \
    }

namespace m3 {

/**
 * The base class of all exceptions. All exceptions have an error-code and collect a backtrace.
 */
class Exception {
    static const size_t MAX_TRACE_DEPTH = 16;
    static const size_t MAX_MSG_SIZE = 256;

public:
    typedef const uintptr_t *backtrace_iterator;

    /**
     * Our verbose terminate handler
     */
    static void terminate_handler();

    /**
     * Constructor
     *
     * @param code the error-code
     */
    explicit Exception(Errors::Code code) noexcept;

    /**
     * Destructor
     */
    virtual ~Exception() {
    }

    /**
     * @return the error-code
     */
    Errors::Code code() const {
        return _code;
    }

    /**
     * @return the error message
     */
    virtual const char *what() const noexcept {
        OStringStream os(msg_buf, sizeof(msg_buf));
        os << "An error occurred: " << Errors::to_string(code()) << " (" << code() << ")";
        return msg_buf;
    }

    /**
     * Writes this exception including backtrace into the given stream.
     *
     * @param os the stream
     */
    void write(OStream &os) const noexcept {
        os << what() << "\n";
        write_backtrace(os);
    }

protected:
    /**
     * Convenience method to write the backtrace to the given stream
     *
     * @param os the stream
     */
    void write_backtrace(OStream &os) const noexcept;

    Errors::Code _code;
    uintptr_t _backtrace[MAX_TRACE_DEPTH];
    static char msg_buf[MAX_MSG_SIZE];
};

/**
 * An exception with a custom message and an optional error code
 */
class MessageException : public Exception {
public:
    explicit MessageException(const String &msg, Errors::Code code = Errors::NONE) noexcept
        : Exception(code),
          _msg(msg) {
    }

    const String &msg() const {
        return _msg;
    }

    const char *what() const noexcept override {
        OStringStream os(msg_buf, sizeof(msg_buf));
        os << _msg;
        if(code() != Errors::NONE)
            os << ": " << Errors::to_string(code()) << " (" << code() << ")";
        return msg_buf;
    }

private:
    String _msg;
};

/**
 * An exception for TCU operations
 */
class TCUException : public Exception {
public:
    explicit TCUException(Errors::Code code) noexcept : Exception(code) {
    }

    const char *what() const noexcept override {
        OStringStream os(msg_buf, sizeof(msg_buf));
        os << "TCU operation failed: " << Errors::to_string(code()) << " (" << code() << ")";
        return msg_buf;
    }
};

/**
 * An exception for failed system calls
 */
class SyscallException : public Exception {
public:
    explicit SyscallException(Errors::Code code, KIF::Syscall::Operation syscall) noexcept
        : Exception(code),
          _syscall(syscall) {
    }

    KIF::Syscall::Operation syscall() const {
        return _syscall;
    }

    const char *what() const noexcept override {
        OStringStream os(msg_buf, sizeof(msg_buf));
        os << "The system call " << _syscall << " failed: " << Errors::to_string(code()) << " ("
           << code() << ")";
        return msg_buf;
    }

private:
    KIF::Syscall::Operation _syscall;
};

}
