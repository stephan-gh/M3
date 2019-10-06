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

#include <m3/ObjCap.h>

#include <utility>

namespace m3 {

class Gate;

/**
 * Represents a DTU endpoint that can be used for communication. This class only serves the purpose
 * to allocate a EP capability and revoke it on destruction. In the meantime, the EP capability can
 * be delegated to someone else.
 */
class EP : public ObjCap {
    friend class Gate;

    static capsel_t alloc_cap(VPE &vpe, epid_t *id);

    explicit EP(capsel_t sel, epid_t id, bool free) noexcept
        : ObjCap(ObjCap::ENDPOINT, sel, KEEP_CAP),
          _id(id),
          _free(free) {
    }

public:
    explicit EP() noexcept;
    EP &operator=(EP &&ep) noexcept;
    EP(EP &&ep) noexcept
        : ObjCap(std::move(ep)),
          _id(ep._id),
          _free(ep._free) {
        ep._free = false;
    }
    ~EP();

    static capsel_t sel_of(VPE &vpe, epid_t ep) noexcept;

    /**
     * Allocate a new endpoint from the current VPE
     *
     * @return the endpoint
     */
    static EP alloc();

    /**
     * Allocate a new endpoint from the given VPE
     *
     * @param vpe the VPE
     * @return the endpoint
     */
    static EP alloc_for(VPE &vpe);

    /**
     * Binds the given endpoint id to a new EP object for the current VPE
     *
     * @param id the endpoint id
     * @return the EP object
     */
    static EP bind(epid_t id) noexcept;

    /**
     * Binds the given endpoint id to a new EP object for the given VPE
     *
     * @param vpe the VPE
     * @param id the endpoint id
     * @return the EP object
     */
    static EP bind_for(VPE &vpe, epid_t id) noexcept;

    /**
     * @return true if the endpoint is valid, i.e., has a selector and endpoint id
     */
    bool valid() const noexcept {
        return sel() != ObjCap::INVALID;
    }

    /**
     * @return the EP id in the DTU
     */
    epid_t id() const noexcept {
        return _id;
    }

private:
    void assign(Gate &gate);
    void free_ep();

    void set_id(epid_t id) noexcept {
        _id = id;
    }

    epid_t _id;
    bool _free;
};

}
