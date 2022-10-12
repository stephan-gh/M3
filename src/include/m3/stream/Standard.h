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

#include <base/stream/Format.h>

#include <m3/stream/FStream.h>

#include <cstdlib>

namespace m3 {

static const fd_t STDIN_FD = 0;
static const fd_t STDOUT_FD = 1;
static const fd_t STDERR_FD = 2;

extern FStream cin;
extern FStream cout;
extern FStream cerr;

/**
 * Convenience function that prints a formatted string to m3::cout.
 * See print_to in Format.h for details on the formatting.
 *
 * @param fmt the format string
 * @param args the arguments
 */
template<typename C, size_t N, detail::StaticString<C, N> S, typename... ARGS>
void print(const detail::CompiledString<C, N, S> &fmt, const ARGS &...args) {
    detail::format_rec<0, 0>(fmt, m3::cout, args...);
}

/**
 * Convenience function that prints a formatted string to m3::cout, appended by a newline.
 * See print_to in Format.h for details on the formatting.
 *
 * @param fmt the format string
 * @param args the arguments
 */
template<typename C, size_t N, detail::StaticString<C, N> S, typename... ARGS>
void println(const detail::CompiledString<C, N, S> &fmt, const ARGS &...args) {
    detail::format_rec<0, 0>(fmt, m3::cout, args...);
    m3::cout.write('\n');
}

/**
 * Convenience function that prints a newline to m3::cout.
 */
static inline void println() {
    m3::cout.write('\n');
}

/**
 * Convenience function that prints a formatted string to m3::cerr.
 * See print_to in Format.h for details on the formatting.
 *
 * @param fmt the format string
 * @param args the arguments
 */
template<typename C, size_t N, detail::StaticString<C, N> S, typename... ARGS>
void eprint(const detail::CompiledString<C, N, S> &fmt, const ARGS &...args) {
    detail::format_rec<0, 0>(fmt, m3::cerr, args...);
}

/**
 * Convenience function that prints a formatted string to m3::cerr, appended by a newline.
 * See print_to in Format.h for details on the formatting.
 *
 * @param fmt the format string
 * @param args the arguments
 */
template<typename C, size_t N, detail::StaticString<C, N> S, typename... ARGS>
void eprintln(const detail::CompiledString<C, N, S> &fmt, const ARGS &...args) {
    detail::format_rec<0, 0>(fmt, m3::cerr, args...);
    m3::cerr.write('\n');
}

/**
 * Convenience function that prints a newline to m3::cerr.
 */
static inline void eprintln() {
    m3::cerr.write('\n');
}

/**
 * Convenience function that prints a formatted string to m3::cerr, appended by a newline, and exits
 * the program with exit code 1. See print_to in Format.h for details on the formatting.
 *
 * @param fmt the format string
 * @param args the arguments
 */
template<typename C, size_t N, detail::StaticString<C, N> S, typename... ARGS>
void exitmsg(const detail::CompiledString<C, N, S> &fmt, const ARGS &...args) {
    detail::format_rec<0, 0>(fmt, m3::cerr, args...);
    m3::cerr.write('\n');
    ::exit(1);
}

}
