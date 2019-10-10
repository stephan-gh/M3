/*
 * Copyright (C) 2016-2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/PEDesc.h>

#include <m3/ObjCap.h>

#include <utility>

namespace m3 {

/**
 * Represents a processing element.
 */
class PE : public ObjCap {
    explicit PE(capsel_t sel, const PEDesc &desc, uint flags) noexcept
        : ObjCap(ObjCap::PE, sel, flags),
          _desc(desc) {
    }

public:
    PE(PE &&pe) noexcept
        : ObjCap(std::move(pe)),
          _desc(pe._desc) {
        pe.flags(KEEP_CAP);
    }
    ~PE();

    /**
     * Allocate a new processing element
     *
     * @param desc the PE description
     * @return the PE object
     */
    static PE alloc(const PEDesc &desc);

    /**
     * Binds a PE object to the given selector and PE description
     *
     * @param sel the selector
     * @param desc the PE description
     * @return the PE object
     */
    static PE bind(capsel_t sel, const PEDesc &desc) {
        return PE(sel, desc, KEEP_CAP);
    }

    /**
     * @return the description of the PE
     */
    const PEDesc &desc() const noexcept {
        return _desc;
    }

private:
    PEDesc _desc;
};

}
