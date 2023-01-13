/*
 * Copyright (C) 2022 Nils Asmussen, Barkhausen Institut
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
#include <base/stream/FormatImpl.h>
#include <base/stream/FormatSpecs.h>
#include <base/stream/OStringStream.h>
#include <base/util/Option.h>

#include <string>
#include <tuple>

namespace m3 {

/**
 * User-defined literal that takes a static string and converts it into an instance of
 * CompiledString, so that we can parse it at compile time.
 *
 * @return the CompiledString
 */
template<detail::StaticString STR>
constexpr auto operator""_cf() {
    using char_t = detail::remove_cvref_t<decltype(STR.data[0])>;
    return detail::CompiledString<char_t, sizeof(STR.data) / sizeof(char_t), STR>();
}

/**
 * The character formatter
 */
struct CharFormatter {
    void format(OStream &os, const FormatSpecs &, char val) const {
        os.write(val);
    }
};

/**
 * Base class for all signed-integer formatters
 */
template<typename T>
struct SignedFormatter {
    void format(OStream &os, const FormatSpecs &fmt, const T &val) const {
        os.write_signed_fmt(static_cast<llong>(val), fmt);
    }
};

/**
 * Base class for all unsigned-integer formatters
 */
template<typename T>
struct UnsignedFormatter {
    void format(OStream &os, const FormatSpecs &fmt, const T &val) const {
        os.write_unsigned_fmt(static_cast<ullong>(val), fmt);
    }
};

/**
 * Base class for all pointer formatters
 */
template<typename T>
struct PointerFormatter {
    void format(OStream &os, const FormatSpecs &, T val) const {
        os.write_pointer(reinterpret_cast<uintptr_t>(val));
    }
};

/**
 * Base class for all float formatters
 */
template<typename T>
struct FloatFormatter {
    void format(OStream &os, const FormatSpecs &fmt, const T &val) const {
        os.write_float_fmt(val, fmt);
    }
};

/**
 * Base class for all string formatters
 */
template<typename T>
struct StringFormatter {
    void format(OStream &os, const FormatSpecs &fmt, const T val) const {
        os.write_string_fmt(val, fmt);
    }
};

/**
 * Base class for all std::string formatters
 */
template<typename C>
struct Formatter<std::basic_string<C>> {
    void format(OStream &os, const FormatSpecs &fmt, const std::basic_string<C> &val) const {
        os.write_string_fmt(val.c_str(), fmt);
    }
};

/**
 * Base class for all std::string_view formatters
 */
template<typename C>
struct Formatter<std::basic_string_view<C>> {
    void format(OStream &os, const FormatSpecs &, const std::basic_string_view<C> &val) const {
        os.write_string_view(val);
    }
};

template<>
struct Formatter<bool> : public SignedFormatter<bool> {};
template<>
struct Formatter<char> : public CharFormatter {};
template<>
struct Formatter<short> : public SignedFormatter<short> {};
template<>
struct Formatter<int> : public SignedFormatter<int> {};
template<>
struct Formatter<long> : public SignedFormatter<long> {};
template<>
struct Formatter<long long> : public SignedFormatter<long long> {};
template<>
struct Formatter<unsigned char> : public UnsignedFormatter<unsigned char> {};
template<>
struct Formatter<unsigned short> : public UnsignedFormatter<unsigned short> {};
template<>
struct Formatter<unsigned int> : public UnsignedFormatter<unsigned int> {};
template<>
struct Formatter<unsigned long> : public UnsignedFormatter<unsigned long> {};
template<>
struct Formatter<unsigned long long> : public UnsignedFormatter<unsigned long long> {};
template<>
struct Formatter<float> : public FloatFormatter<float> {};
template<>
struct Formatter<void *> : public PointerFormatter<void *> {};
template<>
struct Formatter<const void *> : public PointerFormatter<const void *> {};
template<>
struct Formatter<char *> : public StringFormatter<char *> {};
template<>
struct Formatter<const char *> : public StringFormatter<const char *> {};
template<size_t N>
struct Formatter<char[N]> : public StringFormatter<char[N]> {};
template<size_t N>
struct Formatter<const char[N]> : public StringFormatter<const char[N]> {};

/**
 * Prints the given format string including arguments into the given output stream.
 *
 * The format string is parsed and checked at compile time and produces a series of function calls
 * that write strings, characters, integers, ... into the given output stream at runtime. This
 * approach is therefore comparable to the traditional printf, but type safe and less error prone.
 *
 * For example:
 *   OStringStream os;
 *   print_to(os, "my {1} first {0:#x} test: {:.3}"_cf, 0x1234, "cool", "foobar");
 *   // os.str() == "my cool first 0x1234 test: foo"
 *
 * Note that the format string is expected to have the _cf suffix to use the user-defined literal
 * that transforms the static string into a detail::CompiledString, which can be parsed at compile
 * time.
 *
 * The syntax for the format string is similar to Rust's std::fmt, but slightly simplified. In more
 * detail, the grammar is as follows:
 *   format_string := text [ maybe_format text ] *
 *   maybe_format := '{' '{' | '}' '}' | format
 *   format := '{' [ integer ] [ ':' format_spec ] '}'
 *
 *   format_spec := [[fill]align]['+']['#']['0'][width]['.' precision]type
 *   fill := character
 *   align := '<' | '^' | '>'
 *   width := integer
 *   precision := integer
 *   type := '' | 'x' | 'X' | 'b' | 'o' | 'p'
 *
 * The width can be used to pad the to-be-formatted type to a specified number of characters. This
 * can be combined with the fill character to define which character should be used for padding.
 * Additionally, the alignment (left, right, or center) can be specified.
 *
 * The '+' specifies that signed integers should always be preceded by either a '+' (positive
 * integers) or a '-' (negative integers).
 *
 * The '#' specifies that the base should be printed. The type specifies what base to use: 'x' and
 * 'X' denote hexadecimal (lower and upper case, respectively), 'b' denotes binary, and 'o' denotes
 * octal. Additionally, 'p' can be specified to print it as a pointer.
 *
 * The characters '{' and '}' can be escaped by preceding them with the same character:
 * "{{" and "}}".
 *
 * Custom formatters can be defined either via a non-static member function of the type itself:
 * struct MyType {
 *   void format(OStream &os, const FormatSpecs &fmt) const {}
 * };
 *
 * or alternatively via template specialization of the Formatter type:
 * template<>
 * struct Formatter<typename MyType> {
 *   void format(OStream &os, const FormatSpecs &fmt, const MyType &val) const { ... }
 * };
 *
 * Both format functions can of course use this print_to/format_to function in their implementation.
 *
 * @param os the output stream to write to
 * @param fmt the format string
 * @param args the arguments
 */
template<typename C, size_t N, detail::StaticString<C, N> S, typename... ARGS>
void print_to(OStream &os, const detail::CompiledString<C, N, S> &fmt, const ARGS &...args) {
    detail::format_rec<0, 0>(fmt, os, args...);
}

/**
 * The same as print_to, but additionally prints a newline afterwards.
 *
 * @param os the output stream to write to
 * @param fmt the format string
 * @param args the arguments
 */
template<typename C, size_t N, detail::StaticString<C, N> S, typename... ARGS>
void println_to(OStream &os, const detail::CompiledString<C, N, S> &fmt, const ARGS &...args) {
    detail::format_rec<0, 0>(fmt, os, args...);
    os.write('\n');
}

/**
 * Prints a newline to the given output stream.
 *
 * @param os the output stream to write to
 */
static inline void println_to(OStream &os) {
    os.write('\n');
}

/**
 * An alias for print_to for cases where the name format_to seems more natural than print_to.
 *
 * @param os the output stream to write to
 * @param fmt the format string
 * @param args the arguments
 */
template<typename C, size_t N, detail::StaticString<C, N> S, typename... ARGS>
void format_to(OStream &os, const detail::CompiledString<C, N, S> &fmt, const ARGS &...args) {
    detail::format_rec<0, 0>(fmt, os, args...);
}

/**
 * Creates a string from the given format string and arguments.
 *
 * @param fmt the format string
 * @param args the arguments
 * @return the formatted string
 */
template<typename C, size_t N, detail::StaticString<C, N> S, typename... ARGS>
std::string format(const detail::CompiledString<C, N, S> &fmt, const ARGS &...args) {
    OStringStream os;
    detail::format_rec<0, 0>(fmt, os, args...);
    return os.str();
}

}
