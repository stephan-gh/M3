/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
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

#pragma once

#include <base/Backtrace.h>
#include <base/Env.h>
#include <base/stream/Format.h>
#include <base/stream/Serial.h>

namespace m3 {

/**
 * Convenience function that prints a formatted string to Serial::get(), appended by a newline, and
 * calls abort(). See print_to in Format.h for details on the formatting.
 *
 * @param fmt the format string
 * @param args the arguments
 */
template<typename C, size_t N, detail::StaticString<C, N> S, typename... ARGS>
NORETURN void panic(const detail::CompiledString<C, N, S> &fmt, const ARGS &...args) {
    detail::format_rec<0, 0>(fmt, Serial::get(), args...);
    Serial::get().write('\n');
    Backtrace::print(Serial::get());
    abort();
}

}
