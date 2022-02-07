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

#include <base/Types.h>

namespace m3 {

static constexpr uint _getnextlog2(size_t size, uint shift) {
    return size > (static_cast<size_t>(1) << shift)
            ? shift + 1
            : (shift == 0 ? 0 : _getnextlog2(size, shift - 1));
}
/**
 * Converts <size> to x with 2^x >= <size>. It may be executed at compiletime or runtime,
 */
static constexpr uint getnextlog2(size_t size) {
    return _getnextlog2(size, sizeof(size_t) * 8 - 2);
}

/**
 * Converts <size> to x with 2^x >= <size>. It is always executed at compiletime.
 */
template<size_t SIZE>
struct nextlog2 {
    static constexpr uint val = getnextlog2(SIZE);
};

static_assert(nextlog2<0>::val == 0, "failed");
static_assert(nextlog2<1>::val == 0, "failed");
static_assert(nextlog2<8>::val == 3, "failed");
static_assert(nextlog2<10>::val == 4, "failed");
static_assert(nextlog2<100>::val == 7, "failed");
static_assert(nextlog2<1UL << 31>::val == 31, "failed");
static_assert(nextlog2<(1UL << 30) + 1>::val == 31, "failed");
static_assert(nextlog2<(1UL << (sizeof(size_t) * 8 - 1)) + 1>::val == (sizeof(size_t) * 8 - 1), "failed");

/**
 * Converts the given pointer to a label
 */
static inline label_t ptr_to_label(void *ptr) {
    return static_cast<label_t>(reinterpret_cast<word_t>(ptr));
}

}
