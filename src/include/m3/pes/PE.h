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
    template<typename T>
    struct Quota {
        T total;
        T left;
    };

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
     * @param desc the PE description
     * @return the PE object
     */
    static Reference<PE> alloc(const PEDesc &desc);

    /**
     * Gets a PE with given description.
     *
     * The description is an '|' separated list of properties that will be tried in order. Two
     * special properties are supported:
     * - "own" to denote the own PE (provided that it has support for multiple VPEs)
     * - "clone" to denote a separate PE that is identical to the own PE
     *
     * For other properties, see `desc_with_properties` in PE.cc.
     *
     * Examples:
     * - PE with an arbitrary ISA, but preferred the own: "own|core"
     * - Identical PE, but preferred a separate one: "clone|own"
     * - BOOM core if available, otherwise any core: "boom|core"
     * - BOOM with NIC if available, otherwise a Rocket: "boom+nic|rocket"
     *
     * @param desc the textual description of the PE
     */
    static Reference<PE> get(const char *desc);

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
     * Derives a new PE object from the this by transferring a subset of the resources to the new one
     *
     * @param eps the number of EPs to transfer (-1 = none, share the quota)
     * @param time the time slice length in nanoseconds to transfer (-1 = none, share the quota)
     * @param pts the number of page tables to transfer (-1 = none, share the quota)
     * @return the new PE object
     */
    Reference<PE> derive(uint eps = static_cast<uint>(-1),
                         uint64_t time = static_cast<uint64_t>(-1),
                         uint64_t pts = static_cast<uint64_t>(-1));

    /**
     * @return the description of the PE
     */
    const PEDesc &desc() const noexcept {
        return _desc;
    }

    /**
     * Determines the current quotas for EPs, time, and page tables.
     *
     * @param eps is set to the quota for EPs
     * @param time is set to the quota for time (in nanoseconds)
     * @param pts is set to the quota for page tables
     */
    void quota(Quota<uint> *eps, Quota<uint64_t> *time, Quota<size_t> *pts) const;

    /**
     * Sets the quota of the PE with given selector to specified initial values.
     * This call requires a root PE capability.
     *
     * @param time the time slice length in nanoseconds
     * @param pts the number of page tables
     */
    void set_quota(uint64_t time, uint64_t pts);

private:
    PEDesc _desc;
    bool _free;
};

}
