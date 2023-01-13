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

#include <string>

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
        format_to(os, "An error occurred: {}"_cf, code());
        return msg_buf;
    }

    /**
     * Writes this exception including backtrace into the given stream.
     *
     * @param os the stream
     */
    void write(OStream &os) const noexcept {
        format_to(os, "{}\n"_cf, what());
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
    explicit MessageException(const std::string &msg, Errors::Code code = Errors::SUCCESS) noexcept
        : Exception(code),
          _msg(msg) {
    }

    const std::string &msg() const {
        return _msg;
    }

    const char *what() const noexcept override {
        OStringStream os(msg_buf, sizeof(msg_buf));
        format_to(os, "{}"_cf, _msg);
        if(code() != Errors::SUCCESS)
            format_to(os, ": {}"_cf, code());
        return msg_buf;
    }

private:
    std::string _msg;
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
        format_to(os, "TCU operation failed: {}"_cf, code());
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
        format_to(os, "The system call {} failed: {} ({})"_cf, (int)_syscall,
                  Errors::to_string(code()), code());
        return msg_buf;
    }

private:
    KIF::Syscall::Operation _syscall;
};

/**
 * This function throws an exception and passes a formatted string as its message. That is, you can
 * build the message like in print(). For example:
 * vthrow(Errors::EXISTS, "My exception {}, {} message", 1, 2);
 */
template<typename C, size_t N, detail::StaticString<C, N> S, typename... ARGS>
NORETURN void vthrow(Errors::Code error, const detail::CompiledString<C, N, S> &fmt,
                     const ARGS &...args) {
    OStringStream msg;
    detail::format_rec<0, 0>(fmt, msg, args...);
    throw MessageException(msg.str(), error);
}

}
