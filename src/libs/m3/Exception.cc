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

#include <base/Backtrace.h>

#include <m3/stream/Standard.h>
#include <m3/Exception.h>

namespace m3 {

char Exception::msg_buf[Exception::MAX_MSG_SIZE];

void Exception::terminate_handler() {
    try {
        throw;
    }
    catch(const Exception &e) {
        e.write(cerr);
    }
    catch(...) {
        cerr << "Unhandled exception. Terminating.\n";
    }
    abort();
}

Exception::Exception(Errors::Code code) noexcept
    : _code(code),
      _backtrace() {
    size_t count = Backtrace::collect(_backtrace, MAX_TRACE_DEPTH - 1);
    _backtrace[count] = 0;
}

void Exception::write_backtrace(OStream &os) const noexcept {
    os << "Backtrace:\n";
    for(size_t i = 0; i < MAX_TRACE_DEPTH && _backtrace[i]; ++i)
        os << "\t" << fmt(_backtrace[i], "p") << "\n";
}

}
