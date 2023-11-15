/*
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

#include <base/col/SList.h>

#include <m3/Env.h>
#include <m3/cap/ObjCap.h>

#include <utility>

namespace m3 {

class EPMng;

enum EPFlags {
    STANDARD = 0x1,
    CACHEABLE = 0x2,
};

/**
 * Represents a TCU endpoint that can be used for communication. This class only serves the purpose
 * to allocate a EP capability and revoke it on destruction. In the meantime, the EP capability can
 * be delegated to someone else.
 */
class EP : public SListItem, public ObjCap {
    friend class EPMng;

    explicit EP(capsel_t sel, epid_t id, uint replies, uint flags, uint epflags) noexcept
        : SListItem(),
          ObjCap(ObjCap::ENDPOINT, sel, flags),
          _id(id),
          _replies(replies),
          _flags(epflags) {
    }

public:
    static EP alloc(uint replies = 0);
    static EP alloc_for(capsel_t act, epid_t ep = TOTAL_EPS, uint replies = 0);
    static EP bind(epid_t id) noexcept;

    EP &operator=(EP &&ep) noexcept;
    EP(EP &&ep) noexcept
        : SListItem(std::move(ep)),
          ObjCap(std::move(ep)),
          _id(ep._id),
          _replies(ep._replies),
          _flags(ep._flags) {
    }

    /**
     * @return the EP id in the TCU
     */
    epid_t id() const noexcept {
        return _id;
    }

    /**
     * @return the number of reply slots
     */
    uint replies() const noexcept {
        return _replies;
    }

    /**
     * @return if the EP is a standard EP
     */
    bool is_standard() const noexcept {
        return (_flags & EPFlags::STANDARD) != 0;
    }

private:
    bool is_cacheable() const noexcept {
        return (_flags & EPFlags::CACHEABLE) != 0;
    }

    epid_t _id;
    uint _replies;
    uint _flags;
};

}
