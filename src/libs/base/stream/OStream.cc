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

#include <base/stream/Format.h>
#include <base/stream/OStream.h>
#include <base/util/Digits.h>
#include <base/util/Math.h>

#include <string.h>

namespace m3 {

USED char OStream::_hexchars_big[] = "0123456789ABCDEF";
USED char OStream::_hexchars_small[] = "0123456789abcdef";

USED size_t OStream::write_string(const char *str, size_t limit) {
    const char *begin = str;
    char c;
    while((limit == ~0UL || limit-- > 0) && (c = *str)) {
        write(c);
        str++;
    }
    return static_cast<size_t>(str - begin);
}

USED size_t OStream::write_signed(llong n) {
    size_t res = 0;
    if(n < 0) {
        write('-');
        n = -n;
        res++;
    }

    if(n >= 10)
        res += write_signed(n / 10);
    write('0' + (n % 10));
    return res + 1;
}

USED size_t OStream::write_unsigned(ullong n, uint base, char *digits) {
    size_t res = 0;
    if(n >= base)
        res += write_unsigned(n / base, base, digits);
    write(digits[n % base]);
    return res + 1;
}

size_t OStream::write_pointer(uintptr_t p) {
    if constexpr(sizeof(uintptr_t) == 8)
        return write_unsigned_fmt(p, FormatSpecs::create("#016x"_cf));
    else
        return write_unsigned_fmt(p, FormatSpecs::create("#08x"_cf));
}

size_t OStream::write_string_fmt(const char *s, const FormatSpecs &fmt) {
    size_t count = 0;
    size_t width = 0;

    if(fmt.width > 0)
        width = fmt.precision != ~0UL ? Math::min<size_t>(fmt.precision, strlen(s)) : strlen(s);

    if(fmt.align != FormatSpecs::LEFT && fmt.width > 0) {
        if(fmt.width > width)
            count += write_padding(fmt.width - width, fmt.align, fmt.fill, false);
    }

    count += write_string(s, fmt.precision);

    if(fmt.align != FormatSpecs::RIGHT && fmt.width > 0)
        count += write_padding(fmt.width - width, fmt.align, fmt.fill, true);

    return count;
}

size_t OStream::write_signed_fmt(llong n, const FormatSpecs &fmt) {
    if(fmt.base() != 10)
        return write_unsigned_fmt(static_cast<ullong>(n), fmt);

    size_t count = 0;
    size_t width = fmt.width > 0 ? Digits::count_signed(n, 10) : 0;

    // pad left - fill
    if(fmt.align != FormatSpecs::LEFT && !(fmt.flags & FormatSpecs::ZERO) && fmt.width > 0) {
        if(n > 0 && (fmt.flags & FormatSpecs::SIGN))
            width++;
        if(fmt.width > width)
            count += write_padding(fmt.width - width, fmt.align, fmt.fill, false);
    }

    // prefix
    if(n < 0) {
        write('-');
        count++;
        n = -n;
    }
    else if(n > 0 && (fmt.flags & FormatSpecs::SIGN)) {
        write('+');
        count++;
    }

    // pad left - zeros
    if(fmt.align != FormatSpecs::LEFT && (fmt.flags & FormatSpecs::ZERO) && fmt.width > 0) {
        if(fmt.width > width)
            count += write_padding(fmt.width - width, fmt.align, '0', false);
    }

    // print number
    count += write_signed(n);

    // pad right
    if(fmt.align != FormatSpecs::RIGHT && fmt.width > 0)
        count += write_padding(fmt.width - width, fmt.align, fmt.fill, true);

    return count;
}

size_t OStream::write_unsigned_fmt(ullong u, const FormatSpecs &fmt) {
    size_t count = 0;
    uint base = fmt.base();
    size_t width = fmt.width > 0 ? Digits::count_unsigned(u, base) : 0;
    if(fmt.width > 0 && (fmt.flags & FormatSpecs::ALT))
        width += fmt.repr == FormatSpecs::OCTAL ? 1 : 2;

    // pad left - fill
    if(fmt.align != FormatSpecs::LEFT && !(fmt.flags & FormatSpecs::ZERO) && fmt.width > 0) {
        if(fmt.width > width)
            count += write_padding(fmt.width - width, fmt.align, fmt.fill, false);
    }

    // print base-prefix
    if(fmt.flags & FormatSpecs::ALT) {
        uint base = fmt.base();
        if(base == 16 || base == 8 || base == 2) {
            write('0');
            count++;
        }
        if(base == 2) {
            write('b');
            count++;
        }
        else if(base == 16) {
            char c = fmt.repr == FormatSpecs::HEX_UPPER ? 'X' : 'x';
            write(c);
            count++;
        }
    }

    // pad left - zeros
    if(fmt.align != FormatSpecs::LEFT && (fmt.flags & FormatSpecs::ZERO) && fmt.width > 0) {
        if(fmt.width > width)
            count += write_padding(fmt.width - width, fmt.align, '0', false);
    }

    // print number
    if(fmt.repr == FormatSpecs::HEX_UPPER)
        count += write_unsigned(u, base, _hexchars_big);
    else
        count += write_unsigned(u, base, _hexchars_small);

    // pad right
    if(fmt.align != FormatSpecs::RIGHT && fmt.width > 0)
        count += write_padding(fmt.width - width, fmt.align, fmt.fill, true);

    return count;
}

size_t OStream::write_float_fmt(float d, const FormatSpecs &fmt) {
    size_t c = 0;
    if(d < 0) {
        d = -d;
        write('-');
        c++;
    }

    if(Math::is_nan(d))
        c += write_string("nan");
    else if(Math::is_inf(d))
        c += write_string("inf");
    else {
        // TODO this simple approach does not work in general
        llong val = static_cast<llong>(d);
        c += write_signed(val);
        d -= val;

        write('.');
        c++;

        size_t prec = fmt.precision;
        if(prec == ~0UL)
            prec = 3;
        while(prec-- > 0) {
            d *= 10;
            val = static_cast<long>(d);
            write((val % 10) + '0');
            d -= val;
            c++;
        }
    }
    return c;
}

void OStream::dump(const void *data, size_t size) {
    constexpr auto addr_fmt = FormatSpecs::create("#04x"_cf);
    constexpr auto byte_fmt = FormatSpecs::create("#02x"_cf);
    const uint8_t *bytes = reinterpret_cast<const uint8_t *>(data);
    for(size_t i = 0; i < size; ++i) {
        if((i % 16) == 0) {
            if(i > 0)
                write('\n');
            write_unsigned_fmt(i, addr_fmt);
            write(':');
            write(' ');
        }

        write_unsigned_fmt(bytes[i], byte_fmt);

        if(i + 1 < size)
            write(' ');
    }
    write('\n');
}

size_t OStream::write_padding(size_t count, int align, char c, bool right) {
    if(align == FormatSpecs::CENTER) {
        if(right)
            count++;
        count /= 2;
    }

    size_t res = count;
    while(count-- > 0)
        write(c);
    return res;
}

}
