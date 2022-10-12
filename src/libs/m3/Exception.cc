/*
 * Copyright (C) 2019 Nils Asmussen, Barkhausen Institut
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

#include <m3/Exception.h>
#include <m3/stream/Standard.h>

namespace m3 {

char Exception::msg_buf[Exception::MAX_MSG_SIZE];

void Exception::terminate_handler() {
    static bool term_started = false;
    if(term_started)
        abort();

    term_started = true;
    try {
        throw;
    }
    catch(const Exception &e) {
        e.write(cerr);
    }
    catch(...) {
        eprintln("Unhandled exception. Terminating."_cf);
    }
    abort();
}

Exception::Exception(Errors::Code code) noexcept : _code(code), _backtrace() {
    size_t count = Backtrace::collect(_backtrace, MAX_TRACE_DEPTH - 1);
    _backtrace[count] = 0;
}

void Exception::write_backtrace(OStream &os) const noexcept {
    format_to(os, "Backtrace:\n"_cf);
    for(size_t i = 0; i < MAX_TRACE_DEPTH && _backtrace[i]; ++i)
        format_to(os, "\t{:p}\n"_cf, _backtrace[i]);
}

}
