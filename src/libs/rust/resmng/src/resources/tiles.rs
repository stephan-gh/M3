/*
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

use m3::cell::{Cell, RefCell};
use m3::col::Vec;
use m3::com::MemGate;
use m3::errors::{Code, Error};
use m3::kif::{Perm, TileDesc};
use m3::log;
use m3::rc::Rc;
use m3::syscalls;
use m3::tcu::{EpId, TileId};
use m3::tiles::Tile;

// PMP EPs start at 1, because 0 is reserved for TileMux
const FIRST_FREE_PMP_EP: EpId = 1;

#[derive(Debug)]
struct PMP {
    next_ep: EpId,
    regions: Vec<(MemGate, usize)>,
}

impl PMP {
    fn new() -> Self {
        Self {
            next_ep: FIRST_FREE_PMP_EP,
            regions: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct TileUsage {
    idx: Option<usize>,
    pmp: Rc<RefCell<PMP>>,
    tile: Rc<Tile>,
}

impl TileUsage {
    fn new(idx: usize, tile: Rc<Tile>, pmp: Rc<RefCell<PMP>>) -> Self {
        Self {
            idx: Some(idx),
            pmp,
            tile,
        }
    }

    pub fn new_obj(tile: Rc<Tile>) -> Self {
        Self {
            idx: None,
            pmp: Rc::new(RefCell::new(PMP::new())),
            tile,
        }
    }

    pub fn index(&self) -> Option<usize> {
        self.idx
    }

    pub fn tile_id(&self) -> TileId {
        self.tile.id()
    }

    pub fn tile_obj(&self) -> &Rc<Tile> {
        &self.tile
    }

    pub fn add_mem_region(
        &self,
        mgate: MemGate,
        size: usize,
        set: bool,
        overwrite: bool,
    ) -> Result<(), Error> {
        let mut pmp = self.pmp.borrow_mut();
        if set {
            loop {
                match syscalls::set_pmp(self.tile_obj().sel(), mgate.sel(), pmp.next_ep, overwrite)
                {
                    Err(e) if e.code() == Code::Exists && !overwrite => pmp.next_ep += 1,
                    Err(e) => return Err(e),
                    Ok(_) => break,
                }
            }
            pmp.next_ep += 1;
        }
        pmp.regions.push((mgate, size));
        Ok(())
    }

    pub fn inherit_mem_regions(&self, tile: &TileUsage) -> Result<(), Error> {
        let pmps = tile.pmp.borrow();
        for (mgate, size) in pmps.regions.iter() {
            self.add_mem_region(mgate.derive(0, *size, Perm::RWX)?, *size, true, true)?;
        }
        Ok(())
    }

    pub fn derive(
        &self,
        eps: Option<u32>,
        time: Option<u64>,
        pts: Option<usize>,
    ) -> Result<TileUsage, Error> {
        let tile = self.tile_obj().derive(eps, time, pts)?;
        let _quota = tile.quota().unwrap();
        log!(
            crate::LOG_TILES,
            "Deriving {}: (eps={}, time={}, pts={})",
            self.tile_id(),
            _quota.endpoints(),
            _quota.time(),
            _quota.page_tables(),
        );
        Ok(TileUsage {
            idx: self.idx,
            pmp: self.pmp.clone(),
            tile,
        })
    }
}

struct ManagedTile {
    id: TileId,
    tile: Rc<Tile>,
    pmp: Rc<RefCell<PMP>>,
    users: Cell<u32>,
}

impl ManagedTile {
    fn add_user(&self) -> u32 {
        let old = self.users.get();
        self.users.set(old + 1);
        old
    }

    fn remove_user(&self) -> u32 {
        self.users.replace(self.users.get() - 1)
    }
}

#[derive(Default)]
pub struct TileManager {
    tiles: Vec<ManagedTile>,
}

impl TileManager {
    pub fn count(&self) -> usize {
        self.tiles.len()
    }

    pub fn get(&self, idx: usize) -> Rc<Tile> {
        self.tiles[idx].tile.clone()
    }

    pub fn add(&mut self, tile: Rc<Tile>) {
        self.tiles.push(ManagedTile {
            id: tile.id(),
            tile,
            pmp: Rc::new(RefCell::new(PMP::new())),
            users: Cell::from(0),
        });
    }

    pub fn add_user(&self, usage: &TileUsage) {
        if let Some(idx) = usage.idx {
            if self.tiles[idx].add_user() == 0 {
                log!(
                    crate::LOG_TILES,
                    "Allocating {}: {:?} (eps={})",
                    self.tiles[idx].id,
                    self.tiles[idx].tile.desc(),
                    self.get(idx).quota().unwrap().endpoints(),
                );
            }
        }
    }

    pub fn remove_user(&self, usage: &TileUsage) {
        if let Some(idx) = usage.idx {
            if self.tiles[idx].remove_user() == 1 {
                log!(
                    crate::LOG_TILES,
                    "Freeing {}: {:?}",
                    self.tiles[idx].id,
                    self.tiles[idx].tile.desc()
                );
                // all users are gone; restart with the PMP EPs
                self.tiles[idx].pmp.borrow_mut().next_ep = FIRST_FREE_PMP_EP;
            }
        }
    }

    pub fn find(&self, desc: TileDesc) -> Result<TileUsage, Error> {
        for (id, tile) in self.tiles.iter().enumerate() {
            if tile.users.get() == 0
                && tile.tile.desc().isa() == desc.isa()
                && tile.tile.desc().tile_type() == desc.tile_type()
                && (tile.tile.desc().attr() & desc.attr()) == desc.attr()
            {
                return Ok(TileUsage::new(id, tile.tile.clone(), tile.pmp.clone()));
            }
        }
        log!(crate::LOG_TILES, "Unable to find tile with {:?}", desc);
        Err(Error::new(Code::NotFound))
    }

    pub fn find_with_attr(&self, base: TileDesc, attr: &str) -> Result<TileUsage, Error> {
        for props in attr.split('|') {
            if let Ok(usage) = self.find(base.with_properties(props)) {
                return Ok(usage);
            }
        }
        log!(
            crate::LOG_TILES,
            "Unable to find tile with attributes {}",
            attr
        );
        Err(Error::new(Code::NotFound))
    }
}
