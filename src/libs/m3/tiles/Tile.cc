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

#include <m3/session/ResMng.h>
#include <m3/Exception.h>
#include <m3/Syscalls.h>
#include <m3/tiles/Tile.h>
#include <m3/tiles/Activity.h>

namespace m3 {

constexpr size_t MAX_DESC_LEN = 256;

static TileDesc desc_with_properties(TileDesc desc, const char *props) {
    char props_cpy[MAX_DESC_LEN];
    if(strlen(props) >= MAX_DESC_LEN)
        VTHROW(Errors::NO_SPACE, "Tile description too long");
    strcpy(props_cpy, props);

    auto res = desc;
    char *prop = strtok(props_cpy, "+");
    while(prop != nullptr) {
        if(strcmp(prop, "imem") == 0)
            res = TileDesc(TileType::COMP_IMEM, res.isa(), 0);
        else if(strcmp(prop, "emem") == 0 || strcmp(prop, "vm") == 0)
            res = TileDesc(TileType::COMP_EMEM, res.isa(), 0);
        else if(strcmp(prop, "arm") == 0)
            res = TileDesc(res.type(), TileISA::ARM, 0);
        else if(strcmp(prop, "x86") == 0)
            res = TileDesc(res.type(), TileISA::X86, 0);
        else if(strcmp(prop, "riscv") == 0)
            res = TileDesc(res.type(), TileISA::RISCV, 0);
        else if(strcmp(prop, "rocket") == 0)
            res = TileDesc(res.type(), res.isa(), 0, res.attr() | TileAttr::ROCKET);
        else if(strcmp(prop, "boom") == 0)
            res = TileDesc(res.type(), res.isa(), 0, res.attr() | TileAttr::BOOM);
        else if(strcmp(prop, "nic") == 0)
            res = TileDesc(res.type(), res.isa(), 0, res.attr() | TileAttr::NIC);
        else if(strcmp(prop, "kecacc") == 0)
            res = TileDesc(res.type(), res.isa(), 0, res.attr() | TileAttr::KECACC);
        else if(strcmp(prop, "indir") == 0)
            res = TileDesc(TileType::COMP_IMEM, TileISA::ACCEL_INDIR, 0);
        else if(strcmp(prop, "copy") == 0)
            res = TileDesc(TileType::COMP_IMEM, TileISA::ACCEL_COPY, 0);
        else if(strcmp(prop, "rot13") == 0)
            res = TileDesc(TileType::COMP_IMEM, TileISA::ACCEL_ROT13, 0);
        else if(strcmp(prop, "idedev") == 0)
            res = TileDesc(TileType::COMP_IMEM, TileISA::IDE_DEV, 0);
        else if(strcmp(prop, "nicdev") == 0)
            res = TileDesc(TileType::COMP_IMEM, TileISA::NIC_DEV, 0);
        prop = strtok(NULL, "+");
    }
    return res;
}

Tile::~Tile() {
    if(_free) {
        try {
            Activity::own().resmng()->free_tile(sel());
        }
        catch(...) {
            // ignore
        }
    }
}

Reference<Tile> Tile::alloc(const TileDesc &desc) {
    capsel_t sel = Activity::own().alloc_sel();
    TileDesc res = Activity::own().resmng()->alloc_tile(sel, desc);
    return Reference<Tile>(new Tile(sel, res, KEEP_CAP, true));
}

Reference<Tile> Tile::get(const char *desc) {
    char desc_cpy[MAX_DESC_LEN];
    if(strlen(desc) >= MAX_DESC_LEN)
        VTHROW(Errors::NO_SPACE, "Properties description too long");
    strcpy(desc_cpy, desc);

    auto own = Activity::own().tile();
    char *props = strtok(desc_cpy, "|");
    while(props != nullptr) {
        if(strcmp(props, "own") == 0) {
            if(own->desc().supports_tilemux() && own->desc().has_virtmem())
                return own;
        }
        else if(strcmp(props, "clone") == 0) {
            try {
                return Tile::alloc(own->desc());
            }
            catch(...) {
            }
        }
        else {
            auto base = TileDesc(own->desc().type(), own->desc().isa(), 0);
            try {
                return Tile::alloc(desc_with_properties(base, props));
            }
            catch(...) {
            }
        }
        props = strtok(NULL, "|");
    }
    VTHROW(Errors::NOT_FOUND, "Unable to find tile with " << desc);
}

Reference<Tile> Tile::derive(uint eps, uint64_t time, uint64_t pts) {
    capsel_t sel = Activity::own().alloc_sel();
    Syscalls::derive_tile(this->sel(), sel, eps, time, pts);
    return Reference<Tile>(new Tile(sel, desc(), 0, false));
}

void Tile::quota(Quota<uint> *eps, Quota<uint64_t> *time, Quota<size_t> *pts) const {
    Syscalls::tile_quota(sel(), eps, time, pts);
}

void Tile::set_quota(uint64_t time, uint64_t pts) {
    Syscalls::tile_set_quota(sel(), time, pts);
}

}
