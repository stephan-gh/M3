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

#include <optional>

namespace m3 {

/**
 * The special None type
 */
struct NoneType {};

/**
 * Wrapper type around std::optional inspired by Rust's Option type. Option removes all the magic
 * from std::optional (e.g., implicit conversions), so that we have to use everything explicitly.
 */
template<typename T>
class Option {
public:
    Option(NoneType) noexcept : _inner(std::nullopt) {
    }
    explicit Option(T val) noexcept : _inner(val) {
    }

    Option(const Option &r) = default;
    Option &operator=(const Option &r) = default;

    Option(Option &&r) noexcept : _inner(std::move(r._inner)) {
    }
    Option &operator=(Option &&r) noexcept {
        _inner = std::move(r._inner);
        return *this;
    }

    explicit operator bool() const noexcept {
        return is_some();
    }
    bool is_some() const noexcept {
        return _inner.has_value();
    }
    bool is_none() const noexcept {
        return !_inner.has_value();
    }

    T unwrap() const {
        return _inner.value();
    }
    T unwrap_or(T def) const noexcept {
        return _inner.has_value() ? _inner.value() : def;
    }

private:
    std::optional<T> _inner;
};

/**
 * @return a new Option that represents the Some-variant with given value
 */
template<typename T>
static inline Option<T> Some(T val) {
    return Option(val);
}

/**
 * Represents the None-variant for Option
 */
inline constexpr NoneType None = NoneType();

}
