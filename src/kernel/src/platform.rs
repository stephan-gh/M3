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

use base::cell::LazyReadOnlyCell;
use base::col::{String, Vec};
use base::kif::{boot, TileDesc};
use base::mem::{size_of, GlobAddr};
use base::tcu::{EpId, TileId};
use core::iter;

use crate::arch;

#[cfg(not(target_vendor = "host"))]
pub use arch::platform::rbuf_tilemux;

pub struct KEnv {
    info: boot::Info,
    info_addr: GlobAddr,
    mods: Vec<boot::Mod>,
    tiles: Vec<TileDesc>,
}

impl KEnv {
    pub fn new(
        info: boot::Info,
        info_addr: GlobAddr,
        mods: Vec<boot::Mod>,
        tiles: Vec<TileDesc>,
    ) -> Self {
        KEnv {
            info,
            info_addr,
            mods,
            tiles,
        }
    }
}

pub struct TileIterator {
    id: TileId,
    last: TileId,
}

impl TileIterator {
    pub fn new(id: TileId, last: TileId) -> Self {
        Self { id, last }
    }
}

impl iter::Iterator for TileIterator {
    type Item = TileId;

    fn next(&mut self) -> Option<Self::Item> {
        if self.id <= self.last {
            self.id += 1;
            Some(self.id - 1)
        }
        else {
            None
        }
    }
}

static KENV: LazyReadOnlyCell<KEnv> = LazyReadOnlyCell::default();

pub fn init(args: &[String]) {
    KENV.set(arch::platform::init(args));
}

fn get() -> &'static KEnv {
    KENV.get()
}

pub fn info() -> &'static boot::Info {
    &get().info
}

pub fn info_addr() -> GlobAddr {
    get().info_addr
}
pub fn info_size() -> usize {
    size_of::<boot::Info>()
        + info().mod_count as usize * size_of::<boot::Mod>()
        + info().tile_count as usize * size_of::<boot::Tile>()
        + info().mem_count as usize * size_of::<boot::Mem>()
}

pub fn kernel_tile() -> TileId {
    arch::platform::kernel_tile()
}
#[cfg(target_vendor = "host")]
pub fn tiles() -> &'static [TileDesc] {
    &get().tiles
}
pub fn user_tiles() -> TileIterator {
    arch::platform::user_tiles()
}

pub fn tile_desc(tile: TileId) -> TileDesc {
    get().tiles[tile as usize]
}

pub fn is_shared(tile: TileId) -> bool {
    arch::platform::is_shared(tile)
}

pub fn init_serial(dest: Option<(TileId, EpId)>) {
    arch::platform::init_serial(dest);
}

pub fn mods() -> &'static [boot::Mod] {
    &get().mods
}
