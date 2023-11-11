/*
 * Copyright (C) 2023 Nils Asmussen, Barkhausen Institut
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

#include <base/KIF.h>
#include <base/util/Math.h>

#include <m3/Env.h>

namespace m3 {

/**
 * The manager for the capability selector space
 */
class SelSpace {
    // it's initially 0. make sure it's at least the first usable selector
    explicit SelSpace() : _next(Math::max<uint64_t>(KIF::FIRST_FREE_SEL, env()->first_sel)) {
    }

public:
    /**
     * @return the instance of SelSpace
     */
    static SelSpace &get() {
        return _inst;
    }

    /**
     * @return the next selector that will be used
     */
    capsel_t next_sel() const noexcept {
        return _next;
    }

    /**
     * Allocates capability selectors.
     *
     * @param count the number of selectors
     * @return the first one
     */
    capsel_t alloc_sels(uint count) noexcept {
        _next += count;
        return _next - count;
    }
    capsel_t alloc_sel() noexcept {
        return _next++;
    }

private:
    capsel_t _next;
    static SelSpace _inst;
};

}
