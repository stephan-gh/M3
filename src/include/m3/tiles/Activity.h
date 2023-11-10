/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
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

#include <base/ELF.h>
#include <base/Errors.h>
#include <base/KIF.h>
#include <base/TMIF.h>
#include <base/TileDesc.h>
#include <base/time/Instant.h>
#include <base/util/BitField.h>
#include <base/util/Math.h>
#include <base/util/Reference.h>

#include <m3/ObjCap.h>
#include <m3/com/EPMng.h>
#include <m3/com/Marshalling.h>
#include <m3/com/MemGate.h>
#include <m3/com/SendGate.h>
#include <m3/session/Pager.h>
#include <m3/tiles/KMem.h>
#include <m3/tiles/Tile.h>

#include <functional>
#include <memory>

namespace m3 {

class ResMng;
class OwnActivity;
class ChildActivity;
class ClientSession;

/**
 * Represents an activity on a tile. On general-purpose tiles, the activity executes code on the
 * core. On accelerator/device tiles, the activity uses the logic of the accelerator/device.
 */
class Activity : public ObjCap {
    friend class ClientSession;
    friend class ChildActivity;

    static constexpr size_t DATA_SIZE = 256;

protected:
    explicit Activity(capsel_t sel, uint flags, Reference<class Tile> tile, Reference<KMem> kmem);

public:
    /**
     * @return your own activity
     */
    static OwnActivity &own() noexcept;

    virtual ~Activity();

    /**
     * @return the activity id (for debugging purposes)
     */
    actid_t id() const noexcept {
        return _id;
    }

    /**
     * @return the tile this activity has been assigned to
     */
    const Reference<class Tile> &tile() const noexcept {
        return _tile;
    }

    /**
     * @return the tile description this activity has been assigned to
     */
    const TileDesc &tile_desc() const noexcept {
        return _tile->desc();
    }

    /**
     * @return the pager of this activity (or nullptr)
     */
    Reference<Pager> &pager() noexcept {
        return _pager;
    }

    /**
     * @return the kernel memory quota
     */
    const Reference<KMem> &kmem() const noexcept {
        return _kmem;
    }

    /**
     * Revokes the given range of capabilities from this activity.
     *
     * @param crd the capabilities to revoke
     * @param delonly whether to revoke delegations only
     */
    void revoke(const KIF::CapRngDesc &crd, bool delonly = false);

    /**
     * Creates a new memory-gate for the memory region [addr..addr+size) of this activity's address
     * space with given permissions.
     *
     * @param act the activity
     * @param addr the address (page aligned)
     * @param size the memory size (page aligned)
     * @param perms the permissions (see MemGate::RWX)
     * @return the memory gate
     */
    MemGate get_mem(goff_t addr, size_t size, int perms);

    /**
     * Allocates capability selectors.
     *
     * @param count the number of selectors
     * @return the first one
     */
    capsel_t alloc_sels(uint count) noexcept {
        _next_sel += count;
        return _next_sel - count;
    }
    capsel_t alloc_sel() noexcept {
        return _next_sel++;
    }

protected:
    void mark_caps_allocated(capsel_t sel, uint count) {
        _next_sel = Math::max(_next_sel, sel + count);
    }

    actid_t _id;
    capsel_t _next_sel;
    Reference<class Tile> _tile;
    Reference<KMem> _kmem;
    epid_t _eps_start;
    Reference<Pager> _pager;
    unsigned char _data[DATA_SIZE];
};

}
