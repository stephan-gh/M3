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
#include <base/stream/FormatSpecs.h>
#include <base/stream/OStringStream.h>
#include <base/util/Option.h>

#include <string>
#include <tuple>

namespace m3 {

// the general approach is inspired by libfmt (https://github.com/fmtlib/fmt)

namespace detail {

template<typename C>
class StringView {
private:
    const C *_data;
    size_t _size;

public:
    constexpr StringView() noexcept : _data(nullptr), _size(0) {
    }
    constexpr StringView(const C *s, size_t count) noexcept : _data(s), _size(count) {
    }

    constexpr auto data() const noexcept -> const C * {
        return _data;
    }
    constexpr auto size() const noexcept -> size_t {
        return _size;
    }

    constexpr auto find(C c, size_t pos = 0) const noexcept -> const C * {
        if(pos == _size)
            return _data + _size;
        else if(_data[pos] == c)
            return _data + pos;
        else
            return find(c, pos + 1);
    }

    constexpr auto operator[](size_t pos) const noexcept -> const C & {
        return _data[pos];
    }
};

template<typename T>
using remove_reference_t = typename std::remove_reference<T>::type;
template<typename T>
using remove_cvref_t = typename std::remove_cv<remove_reference_t<T>>::type;

template<typename C, size_t N>
struct StaticString {
    constexpr StaticString(const C (&str)[N]) {
        for(size_t i = 0; i < N; ++i)
            data[i] = str[i];
    }
    C data[N] = {};
};

template<typename C, size_t N, StaticString<C, N> STR>
struct CompiledString {
    using char_type = C;
    explicit constexpr operator StringView<char_type>() const {
        return {STR.data, N - 1};
    }
};

template<int N, typename T, typename... ARGS>
constexpr const auto &get_arg(const T &first, const ARGS &...rest) {
    static_assert(N < 1 + sizeof...(ARGS), "index is out of bounds");
    if constexpr(N == 0)
        return first;
    else
        return get_arg<N - 1>(rest...);
}

template<typename S>
constexpr std::tuple<int, int, size_t> parse_number_rec(S str, size_t pos) {
    if(str[pos] >= '0' && str[pos] <= '9') {
        auto rem = parse_number_rec(str, pos + 1);
        return std::make_tuple((str[pos] - '0') * std::get<1>(rem) + std::get<0>(rem),
                               std::get<1>(rem) * 10, std::get<2>(rem) + 1);
    }
    else
        return std::make_tuple(0, 1, 0);
}

template<typename S>
constexpr std::pair<size_t, int> parse_number(S str, size_t pos = 0) {
    auto res = parse_number_rec(str, pos);
    return std::make_pair(std::get<0>(res), std::get<2>(res));
}

template<size_t POS, typename S>
constexpr Option<FormatSpecs::Align> get_align(S fmt) noexcept {
    constexpr auto str = StringView<typename S::char_type>(fmt);
    if constexpr(str[POS] == '<')
        return Some(FormatSpecs::Align::LEFT);
    else if constexpr(str[POS] == '>')
        return Some(FormatSpecs::Align::RIGHT);
    else if constexpr(str[POS] == '^')
        return Some(FormatSpecs::Align::CENTER);
    return None;
}

template<size_t POS, typename S>
static constexpr std::pair<size_t, FormatSpecs> create_fmtspec_with(S fmt) noexcept {
    constexpr auto str = StringView<typename S::char_type>(fmt);
    if constexpr(get_align<POS>(fmt)) {
        constexpr FormatSpecs::Align align = get_align<POS>(fmt).unwrap_or(FormatSpecs::LEFT);
        return create_fmtspec_with_sign<POS + 1>(fmt, 1, ' ', align);
    }
    else if constexpr(POS + 1 < str.size() && get_align<POS + 1>(fmt)) {
        constexpr char fill = str[POS];
        constexpr FormatSpecs::Align align = get_align<POS + 1>(fmt).unwrap_or(FormatSpecs::LEFT);
        return create_fmtspec_with_sign<POS + 2>(fmt, 2, fill, align);
    }
    else
        return create_fmtspec_with_sign<POS + 0>(fmt, 0, ' ', FormatSpecs::LEFT);
}

template<size_t POS, typename S>
constexpr auto create_fmtspec_with_sign(S fmt, size_t off, char fill,
                                        FormatSpecs::Align align) noexcept
    -> std::pair<size_t, FormatSpecs> {
    constexpr auto str = StringView<typename S::char_type>(fmt);
    if constexpr(str[POS] == '+')
        return create_fmtspec_with_alt<POS + 1>(fmt, off + 1, fill, FormatSpecs::SIGN, align);
    else
        return create_fmtspec_with_alt<POS>(fmt, off, fill, FormatSpecs::NONE, align);
}

template<size_t POS, typename S>
constexpr auto create_fmtspec_with_alt(S fmt, size_t off, char fill, int flags,
                                       FormatSpecs::Align align) noexcept
    -> std::pair<size_t, FormatSpecs> {
    constexpr auto str = StringView<typename S::char_type>(fmt);
    if constexpr(str[POS] == '#')
        return create_fmtspec_with_zero<POS + 1>(fmt, off + 1, fill, flags | FormatSpecs::ALT,
                                                 align);
    else
        return create_fmtspec_with_zero<POS>(fmt, off, fill, flags, align);
}

template<size_t POS, typename S>
constexpr auto create_fmtspec_with_zero(S fmt, size_t off, char fill, int flags,
                                        FormatSpecs::Align align) noexcept
    -> std::pair<size_t, FormatSpecs> {
    constexpr auto str = StringView<typename S::char_type>(fmt);
    if constexpr(str[POS] == '0')
        return create_fmtspec_with_width<POS + 1>(fmt, off + 1, fill, flags | FormatSpecs::ZERO,
                                                  FormatSpecs::RIGHT);
    else
        return create_fmtspec_with_width<POS>(fmt, off, fill, flags, align);
}

template<size_t POS, typename S>
constexpr auto create_fmtspec_with_width(S fmt, size_t off, char fill, int flags,
                                         FormatSpecs::Align align) noexcept
    -> std::pair<size_t, FormatSpecs> {
    constexpr auto str = StringView<typename S::char_type>(fmt);
    constexpr auto width = parse_number(str, POS);
    return create_fmtspec_with_prec<POS + width.second>(fmt, off + width.second, fill, flags, align,
                                                        width.first);
}

template<size_t POS, typename S>
constexpr auto create_fmtspec_with_prec(S fmt, size_t off, char fill, int flags,
                                        FormatSpecs::Align align, size_t width) noexcept
    -> std::pair<size_t, FormatSpecs> {
    constexpr auto str = StringView<typename S::char_type>(fmt);
    if constexpr(str[POS] == '.') {
        constexpr auto prec = parse_number(str, POS + 1);
        return create_fmtspec_with_type<POS + 1 + prec.second>(fmt, off + 1 + prec.second, fill,
                                                               flags, align, width, prec.first);
    }
    else
        return create_fmtspec_with_type<POS>(fmt, off + 0, fill, flags, align, width, ~0UL);
}

template<size_t POS, typename S>
constexpr auto create_fmtspec_with_type(S fmt, size_t off, char fill, int flags,
                                        FormatSpecs::Align align, size_t width,
                                        size_t prec) noexcept -> std::pair<size_t, FormatSpecs> {
    constexpr auto str = StringView<typename S::char_type>(fmt);
    if constexpr(str[POS] == 'x')
        return std::make_pair(off + 1,
                              FormatSpecs(FormatSpecs::HEX_LOWER, fill, flags, align, width, prec));
    else if constexpr(str[POS] == 'X')
        return std::make_pair(off + 1,
                              FormatSpecs(FormatSpecs::HEX_UPPER, fill, flags, align, width, prec));
    else if constexpr(str[POS] == 'o')
        return std::make_pair(off + 1,
                              FormatSpecs(FormatSpecs::OCTAL, fill, flags, align, width, prec));
    else if constexpr(str[POS] == 'b')
        return std::make_pair(off + 1,
                              FormatSpecs(FormatSpecs::BINARY, fill, flags, align, width, prec));
    else if constexpr(str[POS] == 'p')
        return std::make_pair(off + 1,
                              FormatSpecs(FormatSpecs::POINTER, fill, flags, align, width, prec));
    else
        return std::make_pair(off + 0,
                              FormatSpecs(FormatSpecs::DEFAULT, fill, flags, align, width, prec));
}

template<typename A>
concept has_format = requires(OStream &os, A &arg) { arg.format(os, FormatSpecs()); };

template<size_t POS, size_t ARG, typename A, typename S, typename... ARGS>
constexpr void format_with_spec_rec(S fmt, OStream &os, A &arg, const ARGS &...args) {
    constexpr auto str = StringView<typename S::char_type>(fmt);
    using A_raw = remove_cvref_t<A>;

    if constexpr(str[POS] == ':') {
        constexpr auto spec = create_fmtspec_with<POS + 1>(fmt);
        if constexpr(has_format<A>)
            arg.format(os, spec.second);
        else {
            constexpr auto f = Formatter<A_raw>();
            f.format(os, spec.second, arg);
        }
        static_assert(str[POS + 1 + spec.first] == '}', "expected closing brace");
        return format_rec<POS + 1 + spec.first + 1, ARG, S>(fmt, os, args...);
    }
    else {
        if constexpr(has_format<A>)
            arg.format(os, FormatSpecs());
        else {
            constexpr auto f = Formatter<A_raw>();
            f.format(os, FormatSpecs(), arg);
        }
        static_assert(str[POS] == '}', "expected closing brace");
        return format_rec<POS + 1, ARG, S>(fmt, os, args...);
    }
}

template<size_t POS, size_t ARG, typename S, typename... ARGS>
constexpr void format_rec(S fmt, OStream &os, const ARGS &...args) {
    constexpr auto str = StringView<typename S::char_type>(fmt);
    if constexpr(POS >= str.size()) {
        return;
    }
    else if constexpr(str[POS] == '{') {
        if constexpr(str[POS + 1] == '{') {
            os.write('{');
            return format_rec<POS + 2, ARG, S>(fmt, os, args...);
        }
        else if constexpr(str[POS + 1] >= '0' && str[POS + 1] <= '9') {
            constexpr auto argno = parse_number(str, POS + 1);
            auto &arg = get_arg<argno.first>(args...);
            constexpr auto off = POS + 1 + argno.second;
            return format_with_spec_rec<off, ARG, decltype(arg), S>(fmt, os, arg, args...);
        }
        else {
            auto &arg = get_arg<ARG>(args...);
            return format_with_spec_rec<POS + 1, ARG + 1, decltype(arg), S>(fmt, os, arg, args...);
        }
    }
    else if constexpr(str[POS] == '}') {
        static_assert(str[POS + 1] == '}', "unexpected closing brace");
        os.write('}');
        return format_rec<POS + 2, ARG, S>(fmt, os, args...);
    }
    else {
        constexpr auto next_open = str.find('{', POS);
        constexpr auto next_close = str.find('}', POS);
        constexpr auto len = std::min(next_open, next_close) - (str.data() + POS);
        os.write_string_view(std::string_view(str.data() + POS, len));
        return format_rec<POS + len, ARG, S>(fmt, os, args...);
    }
}

}

template<typename S>
constexpr FormatSpecs FormatSpecs::create(S fmt) noexcept {
    return detail::create_fmtspec_with<0>(fmt).second;
}

}
