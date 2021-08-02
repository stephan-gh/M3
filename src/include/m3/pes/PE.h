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

#include <base/util/Reference.h>
#include <base/PEDesc.h>

#include <m3/ObjCap.h>

#include <utility>

namespace m3 {

/**
 * Represents a processing element.
 */
class PE : public ObjCap, public RefCounted {
    explicit PE(capsel_t sel, const PEDesc &desc, uint flags, bool free) noexcept
        : ObjCap(ObjCap::PE, sel, flags),
          RefCounted(),
          _desc(desc),
          _free(free) {
    }

public:
    PE(PE &&pe) noexcept
        : ObjCap(std::move(pe)),
          RefCounted(std::move(pe)),
          _desc(pe._desc),
          _free(pe._free) {
        pe.flags(KEEP_CAP);
        pe._free = false;
    }
    ~PE();

    /**
     * Allocate a new processing element
     *
     * @param name the local name (boot script)
     * @return the PE object
     */
    static Reference<PE> alloc(const char *name);

    /**
     * Binds a PE object to the given selector and PE description
     *
     * @param sel the selector
     * @param desc the PE description
     * @return the PE object
     */
    static Reference<PE> bind(capsel_t sel, const PEDesc &desc) {
        return Reference<PE>(new PE(sel, desc, KEEP_CAP, false));
    }

    /**
     * Derives a new PE object from the this by transferring <eps> endpoints to the new one
     *
     * @param eps the number of EPs to transfer
     * @return the new PE object
     */
    Reference<PE> derive(uint eps);

    /**
     * @return the description of the PE
     */
    const PEDesc &desc() const noexcept {
        return _desc;
    }

    /**
     * @return the number of available EPs
     */
    uint quota() const;

private:
    PEDesc _desc;
    bool _free;
};

}
