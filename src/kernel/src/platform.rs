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

use base::cell::{LazyReadOnlyCell, StaticCell};
use base::cfg;
use base::col::Vec;
use base::env;
use base::goff;
use base::kif::{self, boot, Perm, TileDesc, TileISA, TileType};
use base::mem::{size_of, GlobAddr};
use base::tcu::{ActId, EpId, TileId, TCU, UNLIM_CREDITS};
use base::vec;
use core::iter;

use crate::args;
use crate::ktcu;
use crate::mem::{self, MemMod, MemType};
use crate::tiles::KERNEL_ID;

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
static LAST_TILE: StaticCell<TileId> = StaticCell::new(0);

fn get() -> &'static KEnv {
    KENV.get()
}

pub fn mods() -> &'static [boot::Mod] {
    &get().mods
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
    env::data().tile_id as TileId
}
pub fn user_tiles() -> TileIterator {
    TileIterator::new(kernel_tile() + 1, LAST_TILE.get())
}

pub fn tile_desc(tile: TileId) -> TileDesc {
    get().tiles[tile as usize]
}

pub fn is_shared(tile: TileId) -> bool {
    tile_desc(tile).is_programmable()
}

pub fn init() {
    // read kernel env
    let addr = GlobAddr::new(env::data().kenv);
    let mut offset = addr.offset();
    let info: boot::Info = ktcu::read_obj(addr.tile(), offset);
    offset += size_of::<boot::Info>() as goff;

    // read boot modules
    let mut mods: Vec<boot::Mod> = vec![boot::Mod::default(); info.mod_count as usize];
    ktcu::read_slice(addr.tile(), offset, &mut mods);
    offset += info.mod_count as goff * size_of::<boot::Mod>() as goff;

    // read tiles
    let mut tiles: Vec<TileDesc> = vec![TileDesc::default(); info.tile_count as usize];
    ktcu::read_slice(addr.tile(), offset, &mut tiles);
    offset += info.tile_count as goff * size_of::<TileDesc>() as goff;

    // read memory regions
    let mut mems: Vec<boot::Mem> = vec![boot::Mem::default(); info.mem_count as usize];
    ktcu::read_slice(addr.tile(), offset, &mut mems);

    // build new info for user tiles
    let mut uinfo = boot::Info {
        mod_count: info.mod_count,
        tile_count: info.tile_count,
        mem_count: info.mem_count,
        serv_count: 0,
    };

    let mut umems = Vec::new();
    let mut utiles = Vec::new();

    // register memory modules
    let mut kmem_idx = 0;
    let mut mem = mem::borrow_mut();
    for (i, tile) in tiles.iter().enumerate() {
        if tile.tile_type() == TileType::MEM {
            // the first memory module hosts the FS image and other stuff
            if kmem_idx == 0 {
                let avail = mems[kmem_idx].size();
                if avail <= args::get().kmem as goff {
                    panic!("Not enough DRAM for kernel memory ({})", args::get().kmem);
                }

                // file system image
                let mut used = tile.mem_size() as goff - avail;
                mem.add(MemMod::new(MemType::OCCUPIED, i as TileId, 0, used));
                umems.push(boot::Mem::new(
                    GlobAddr::new_with(i as TileId, 0),
                    used,
                    true,
                ));

                // kernel memory
                let kmem =
                    MemMod::new(MemType::KERNEL, i as TileId, used, args::get().kmem as goff);
                used += args::get().kmem as goff;
                // configure EP to give us access to this range of physical memory
                ktcu::config_local_ep(1, |regs| {
                    ktcu::config_mem(
                        regs,
                        KERNEL_ID,
                        kmem.addr().tile(),
                        kmem.addr().offset(),
                        kmem.capacity() as usize,
                        Perm::RW,
                    );
                });
                mem.add(kmem);

                // root memory
                mem.add(MemMod::new(
                    MemType::ROOT,
                    i as TileId,
                    used,
                    cfg::FIXED_ROOT_MEM as goff,
                ));
                used += cfg::FIXED_ROOT_MEM as goff;

                // user memory
                let user_size = core::cmp::min((1 << 30) - cfg::PAGE_SIZE as goff, avail);
                mem.add(MemMod::new(MemType::USER, i as TileId, used, user_size));
                umems.push(boot::Mem::new(
                    GlobAddr::new_with(i as TileId, used),
                    user_size - args::get().kmem as goff,
                    false,
                ));
            }
            else {
                let user_size = core::cmp::min((1 << 30) - cfg::PAGE_SIZE, tile.mem_size());
                mem.add(MemMod::new(
                    MemType::USER,
                    i as TileId,
                    0,
                    user_size as goff,
                ));
                umems.push(boot::Mem::new(
                    GlobAddr::new_with(i as TileId, 0),
                    user_size as goff,
                    false,
                ));
            }
            kmem_idx += 1;
        }
        else {
            if kmem_idx > 0 {
                panic!("All memory tiles have to be last");
            }

            LAST_TILE.set(i as TileId);

            if i > 0 {
                assert!(kernel_tile() == 0);
                utiles.push(boot::Tile::new(i as u32, *tile));
            }
        }
    }

    // write-back boot info
    let mut uoffset = addr.offset();
    uinfo.tile_count = utiles.len() as u64;
    uinfo.mem_count = umems.len() as u64;
    ktcu::write_slice(addr.tile(), uoffset, &[uinfo]);
    uoffset += size_of::<boot::Info>() as goff;
    uoffset += info.mod_count as goff * size_of::<boot::Mod>() as goff;

    // write-back user tiles
    ktcu::write_slice(addr.tile(), uoffset, &utiles);
    uoffset += uinfo.tile_count as goff * size_of::<boot::Tile>() as goff;

    // write-back user memory regions
    ktcu::write_slice(addr.tile(), uoffset, &umems);

    KENV.set(KEnv::new(info, addr, mods, tiles));
}

pub fn init_serial(dest: Option<(TileId, EpId)>) {
    if env::data().platform == env::Platform::HW.val {
        let (tile, ep) = dest.unwrap_or((0, 0));
        let serial = GlobAddr::new(env::data().kenv + 16 * 1024 * 1024);
        let tile_modid = TCU::tileid_to_nocid(tile);
        ktcu::write_slice(serial.tile(), serial.offset(), &[
            tile_modid as u64,
            ep as u64,
        ]);
    }
    else if let Some(ser_tile) = user_tiles().find(|i| tile_desc(*i).isa() == TileISA::SERIAL_DEV)
    {
        if let Some((tile, ep)) = dest {
            ktcu::config_remote_ep(ser_tile, 4, |regs| {
                let act = kif::tilemux::ACT_ID as ActId;
                ktcu::config_send(regs, act, 0, tile, ep, cfg::SERIAL_BUF_ORD, UNLIM_CREDITS);
            })
            .unwrap();
        }
        else {
            ktcu::invalidate_ep_remote(ser_tile, 4, true).unwrap();
        }
    }
}
