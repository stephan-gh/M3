/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
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

//! Contains the activity abstraction

use core::fmt;

use crate::cap::{Capability, Selector};
use crate::cell::LazyReadOnlyCell;
use crate::client::{Pager, ResMng};
use crate::col::Vec;
use crate::com::MemGate;
use crate::errors::Error;
use crate::kif;
use crate::kif::{CapRngDesc, TileDesc};
use crate::mem::{GlobOff, VirtAddr};
use crate::rc::Rc;
use crate::syscalls;
use crate::tcu::{ActId, EpId, TileId};
use crate::tiles::{KMem, OwnActivity, Tile};

/// Represents an activity on a tile
///
/// On general-purpose tiles, the activity executes code on the core. On accelerator/device tiles,
/// the activity uses the logic of the accelerator/device.
///
/// [`Activity`] is the "base class" of all activities, which come in two flavors: [`OwnActivity`]
/// that represents the own activity and [`ChildActivity`](`crate::tiles::ChildActivity`) that is
/// used to create child activities. Both share common properties such as an id, a capability, a
/// [`Tile`] etc., which are part of [`Activity`]. Both [`OwnActivity`] and
/// [`ChildActivity`](`crate::tiles::ChildActivity`) implement `Deref` to [`Activity`] to make the
/// common properties accessible.
pub struct Activity {
    pub(crate) id: ActId,
    pub(crate) cap: Capability,
    pub(crate) rmng: Option<ResMng>, // close the connection resource manager at last
    pub(crate) tile: Rc<Tile>,
    pub(crate) kmem: Rc<KMem>,
    pub(crate) eps_start: EpId,
    pub(crate) pager: Option<Pager>,
    pub(crate) data: Vec<u64>,
}

static OWN: LazyReadOnlyCell<OwnActivity> = LazyReadOnlyCell::default();

impl Activity {
    pub(crate) fn new_act(cap: Capability, tile: Rc<Tile>, kmem: Rc<KMem>) -> Self {
        Activity {
            id: 0,
            cap,
            tile,
            rmng: None,
            eps_start: 0,
            pager: None,
            kmem,
            data: Vec::default(),
        }
    }

    /// Returns the own activity
    pub fn own() -> &'static OwnActivity {
        OWN.get()
    }

    /// Returns the capability selector.
    pub fn sel(&self) -> Selector {
        self.cap.sel()
    }

    /// Returns the ID of the activity (for debugging purposes)
    pub fn id(&self) -> ActId {
        self.id
    }

    /// Returns the description of the tile the activity has been assigned to.
    pub fn tile(&self) -> &Rc<Tile> {
        &self.tile
    }

    /// Returns the description of the tile the activity has been assigned to.
    pub fn tile_desc(&self) -> TileDesc {
        self.tile.desc()
    }

    /// Returns the id of the tile the activity has been assigned to.
    pub fn tile_id(&self) -> TileId {
        self.tile.id()
    }

    /// Returns a reference to the activity's kernel memory.
    pub fn kmem(&self) -> &Rc<KMem> {
        &self.kmem
    }

    /// Returns a reference to the activity's pager.
    pub fn pager(&self) -> Option<&Pager> {
        self.pager.as_ref()
    }

    /// Revokes the given capability range from `self`.
    ///
    /// If `del_only` is true, only the delegations are revoked, that is, the capability is not
    /// revoked from `self`.
    pub fn revoke(&self, crd: CapRngDesc, del_only: bool) -> Result<(), Error> {
        syscalls::revoke(self.sel(), crd, !del_only)
    }

    /// Creates a new [`MemGate`] that refers to the address region `virt`..`virt`+`size` in the
    /// virtual address space of this activity.
    ///
    /// The given region in virtual memory must be physically contiguous and page aligned. See
    /// [`MemGate`] for a more detailed explanation of how that works.
    pub fn get_mem(
        &self,
        virt: VirtAddr,
        size: GlobOff,
        perms: kif::Perm,
    ) -> Result<MemGate, Error> {
        MemGate::new_foreign(self.sel(), virt, size, perms)
    }
}

impl fmt::Debug for Activity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "Activity[sel: {}, tile: {:?}]", self.sel(), self.tile())
    }
}

pub(crate) fn init() {
    OWN.set(OwnActivity::new());
}
