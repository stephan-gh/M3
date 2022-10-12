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
#include <base/util/Option.h>

#include <string_view>
#include <tuple>

namespace m3 {

/**
 * The class for custom formatters. If a custom formatting is desired for a type, template
 * specialization can be used to do so:
 * template<>
 * struct Formatter<typename MyType> {
 *   void format(OStream &os, const FormatSpecs &fmt, const MyType &val) const { ... }
 * };
 *
 * This will be picked up by format_rec below to format a type accordingly. Note that this class has
 * no members to detect whether formatting is not supported for a type (and to let the compiler
 * produce a reasonably readable error message).
 */
template<typename T>
struct Formatter {};

/**
 * The formatting specifications, which are created at compile time and then passed to one of the
 * formatting functions in OStream to format a type at runtime.
 */
struct FormatSpecs {
    /**
     * The representation that should be used to print the type
     */
    enum Repr {
        DEFAULT,
        HEX_LOWER,
        HEX_UPPER,
        OCTAL,
        BINARY,
        POINTER,
    };

    /**
     * The alignment of the type
     */
    enum Align {
        LEFT,
        CENTER,
        RIGHT,
    };

    /**
     * Other formatting flags
     */
    enum Flags {
        NONE = 0,
        ALT = 1,
        ZERO = 2,
        SIGN = 4,
    };

    /**
     * Creates the default formatting specification, i.e., the instance that is used if no "{:...}"
     * is used to customize the formatting.
     */
    constexpr FormatSpecs() noexcept : FormatSpecs(DEFAULT, ' ', NONE, LEFT, 0, ~0UL) {
    }

    /**
     * Creates a formatting specification from given arguments
     *
     * @param _repr the representation
     * @param _fill the fill character (used when width is set)
     * @param _flags the formatting flags
     * @param _align the alignment (used when width is set)
     * @param _width the width of the formatting
     * @param _precision the precision (has different meanings, depending on the type)
     */
    constexpr FormatSpecs(Repr _repr, char _fill, int _flags, Align _align, size_t _width,
                          size_t _precision) noexcept
        : repr(_repr),
          fill(_fill),
          flags(_flags),
          align(_align),
          width(_width),
          precision(_precision) {
    }

    /**
     * Creates a new instance of this type at compile-time from the given format string portion
     * (the part between the ':' and the '}' in a format string like "{:#x}").
     *
     * @param fmt the format string portion
     * @return the format specification instance
     */
    template<typename S>
    static constexpr FormatSpecs create(S fmt) noexcept;

    /**
     * @return the base an integer should be printed in
     */
    constexpr uint base() const noexcept {
        switch(repr) {
            case HEX_LOWER: return 16;
            case HEX_UPPER: return 16;
            case POINTER: return 16;
            case OCTAL: return 8;
            case BINARY: return 2;
            default: return 10;
        }
    }

    Repr repr;
    char fill;
    int flags;
    Align align;
    size_t width;
    size_t precision;
};

}
