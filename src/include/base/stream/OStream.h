/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
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
#include <base/stream/IOSBase.h>

#include <stdarg.h>
#include <string_view>

namespace m3 {

struct FormatSpecs;

/**
 * The output-stream is used to write formatted output to various destinations. Subclasses have
 * to implement the method to actually write a character.
 *
 * This class provides methods to format different types and write them into the stream. However,
 * these methods are not designed to be used directly, but instead the type-safe format/printing
 * functions should be used. See Format.h for details.
 */
class OStream : public virtual IOSBase {
public:
    explicit OStream() : IOSBase() {
    }
    virtual ~OStream() {
    }

    OStream(const OStream &) = delete;
    OStream &operator=(const OStream &) = delete;

    /**
     * Writes the given character into the stream.
     *
     * @param c the character
     */
    virtual void write(char c) = 0;

    /**
     * Produces a hexdump of the given data.
     *
     * @param data the data
     * @param size the number of bytes
     */
    void dump(const void *data, size_t size);

    /**
     * Writes the given string view into the stream.
     */
    void write_string_view(const std::string_view &str) {
        for(auto it = str.cbegin(); it != str.cend(); ++it)
            write(*it);
    }

    /**
     * Writes the given string/signed integer/unsigned integer/float according to <fmt> into the
     * stream.
     *
     * @param v the value to write
     * @param fmt the format specifications
     * @return the number of written bytes
     */
    size_t write_string_fmt(const char *v, const FormatSpecs &fmt);
    size_t write_signed_fmt(llong v, const FormatSpecs &fmt);
    size_t write_unsigned_fmt(ullong v, const FormatSpecs &fmt);
    size_t write_float_fmt(float v, const FormatSpecs &fmt);

    /**
     * Writes the given string into the stream, optionally with a limited length.
     *
     * @param str the string
     * @param limit the number of characters to write
     * @return the number of written bytes
     */
    size_t write_string(const char *str, size_t limit = ~0UL);

    /**
     * Writes the given signed integer into the stream
     *
     * @param n the signed integer
     * @return the number of written bytes
     */
    size_t write_signed(llong n);

    /**
     * Writes the given unsigned integer into the stream
     *
     * @param n the unsigned integer
     * @param base the base (2..16)
     * @param digits the characters to use as digits
     * @return the number of written bytes
     */
    size_t write_unsigned(ullong n, uint base, char *digits);

    /**
     * Writes the given pointer into the stream
     *
     * @param p the pointer
     * @return the number of written bytes
     */
    size_t write_pointer(uintptr_t p);

private:
    size_t write_padding(size_t count, int align, char c, bool right);

    static char _hexchars_big[];
    static char _hexchars_small[];
};

}
