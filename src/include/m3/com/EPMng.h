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

#include <base/Common.h>
#include <base/Config.h>
#include <base/DTU.h>
#include <base/Errors.h>

#include <assert.h>

namespace m3 {

class Gate;
class VPE;

/**
 * The endpoint manager allows us to have more gates than endpoints by multiplexing
 * the endpoints among the gates.
 */
class EPMng {
    friend class VPE;

public:
    explicit EPMng(bool mux);

    /**
     * Allocates a new endpoint and reserves it, that is, excludes it from multiplexing. Note that
     * this can fail if a send gate with missing credits is using this EP.
     *
     * @return the endpoint id
     */
    epid_t alloc_ep();

    /**
     * Frees the given endpoint
     *
     * @param id the endpoint id
     */
    void free_ep(epid_t id) noexcept;

    /**
     * Configures an endpoint for the given gate. If necessary, a victim will be picked and removed
     * from an endpoint.
     *
     * @param gate the gate
     */
    void switch_to(Gate *gate);

    /**
     * Removes <gate> from the endpoint it is configured on, if any. If <invalidate> is true, the
     * kernel will invalidate the endpoint as well.
     *
     * @param gate the gate
     * @param invalidate whether to invalidate it, too
     */
    void remove(Gate *gate, bool invalidate) noexcept;

    /**
     * Resets the state of the EP switcher.
     */
    void reset(uint64_t eps) noexcept;

private:
    uint64_t reserved() const noexcept {
        return _eps;
    }
    bool is_ep_free(epid_t id) const noexcept;
    bool is_in_use(epid_t ep) const noexcept;
    epid_t select_victim();
    void activate(epid_t ep, capsel_t newcap);

    uint64_t _eps;
    epid_t _next_victim;
    Gate **_gates;
};

}
