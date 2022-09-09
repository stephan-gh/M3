/*
 * Copyright (C) 2020-2022 Nils Asmussen, Barkhausen Institut
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

use base::cell::{LazyStaticRefCell, RefMut};
use base::col::Vec;
use base::kif;
use base::tcu::TileId;

use crate::ktcu;
use crate::platform;
use crate::tiles::TileMux;

static INST: LazyStaticRefCell<Vec<TileMux>> = LazyStaticRefCell::default();

pub fn init() {
    deprivilege_tiles();

    let mut muxes = Vec::new();
    for tile in platform::user_tiles() {
        muxes.push(TileMux::new(tile));
    }
    INST.set(muxes);
}

pub fn tilemux(tile: TileId) -> RefMut<'static, TileMux> {
    assert!(tile > 0);
    RefMut::map(INST.borrow_mut(), |tiles| &mut tiles[tile as usize - 1])
}

pub fn find_tile(tiledesc: &kif::TileDesc) -> Option<TileId> {
    for tile in platform::user_tiles() {
        if platform::tile_desc(tile).isa() == tiledesc.isa()
            || platform::tile_desc(tile).tile_type() == tiledesc.tile_type()
        {
            return Some(tile);
        }
    }

    None
}

fn deprivilege_tiles() {
    for tile in platform::user_tiles() {
        ktcu::deprivilege_tile(tile).expect("Unable to deprivilege tile");
    }
}
