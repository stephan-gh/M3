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

use base::envdata;

use core::fmt;
use core::ops::Deref;

use crate::arch;
use crate::cap::{CapFlags, Capability, Selector};
use crate::cell::{Cell, Ref, RefCell, RefMut};
use crate::com::EpMng;
use crate::errors::Error;
use crate::kif;
use crate::rc::Rc;
use crate::session::{Pager, ResMng};
use crate::tcu::{EpId, TileId, INVALID_EP, TCU};
use crate::tiles::{Activity, KMem, StateDeserializer, Tile};
use crate::time::TimeDuration;
use crate::tmif;
use crate::vfs::{FileTable, MountTable};

/// Represents the own activity.
pub struct OwnActivity {
    base: Activity,
    pub(crate) next_sel: Cell<Selector>,
    epmng: RefCell<EpMng>,
    files: RefCell<FileTable>,
    mounts: RefCell<MountTable>,
}

impl OwnActivity {
    pub(crate) fn new() -> Self {
        let env = arch::env::get();
        OwnActivity {
            base: Activity {
                id: env.activity_id(),
                cap: Capability::new(kif::SEL_ACT, CapFlags::KEEP_CAP),
                tile: Rc::new(Tile::new_bind(
                    env.tile_id() as TileId,
                    env.tile_desc(),
                    kif::SEL_TILE,
                )),
                eps_start: env.first_std_ep(),
                rmng: env.load_rmng(),
                pager: env.load_pager(),
                data: env.load_data(),
                kmem: Rc::new(KMem::new(kif::SEL_KMEM)),
            },
            next_sel: Cell::from(env.load_first_sel()),
            epmng: RefCell::new(EpMng::default()),
            // mounts first; files depend on mounts
            mounts: RefCell::new(env.load_mounts()),
            files: RefCell::new(env.load_fds()),
        }
    }

    /// Puts the own activity to sleep until the next message arrives
    #[inline(always)]
    pub fn sleep(&self) -> Result<(), Error> {
        self.sleep_for(TimeDuration::MAX)
    }

    /// Puts the own activity to sleep until the next message arrives or `timeout` time has passed.
    #[inline(always)]
    pub fn sleep_for(&self, timeout: TimeDuration) -> Result<(), Error> {
        if envdata::get().platform != envdata::Platform::HOST.val
            && (arch::env::get().shared() || timeout != TimeDuration::MAX)
        {
            let timeout = match timeout {
                TimeDuration::MAX => None,
                t => Some(t),
            };
            return tmif::wait(None, None, timeout);
        }
        if envdata::get().platform != envdata::Platform::HW.val {
            let timeout = match timeout {
                TimeDuration::MAX => None,
                t => Some(t.as_nanos() as u64),
            };
            return TCU::wait_for_msg(INVALID_EP, timeout);
        }
        Ok(())
    }

    /// Puts the own activity to sleep until the next message arrives on the given EP
    pub fn wait_for(
        &self,
        ep: Option<EpId>,
        irq: Option<tmif::IRQId>,
        timeout: Option<TimeDuration>,
    ) -> Result<(), Error> {
        if arch::env::get().shared() {
            return tmif::wait(ep, irq, timeout);
        }
        if envdata::get().platform != envdata::Platform::HW.val {
            if let Some(ep) = ep {
                let timeout = timeout.map(|t| t.as_nanos() as u64);
                return TCU::wait_for_msg(ep, timeout);
            }
        }
        Ok(())
    }

    /// Returns a mutable reference to the file table of this activity.
    pub fn files(&self) -> RefMut<'_, FileTable> {
        self.files.borrow_mut()
    }

    /// Returns a mutable reference to the mount table of this activity.
    pub fn mounts(&self) -> RefMut<'_, MountTable> {
        self.mounts.borrow_mut()
    }

    /// Returns a source for the activity-local data
    ///
    /// The source provides access to the activity-local data that has been transmitted to this
    /// activity from its parent during [`ChildActivity::run`](crate::tiles::ChildActivity::run) and
    /// [`ChildActivity::exec`](crate::tiles::ChildActivity::exec).
    pub fn data_source(&self) -> StateDeserializer<'_> {
        StateDeserializer::new(&self.data)
    }

    /// Returns a reference to the endpoint manager
    pub fn epmng(&self) -> Ref<'_, EpMng> {
        self.epmng.borrow()
    }

    /// Returns a mutable reference to the endpoint manager
    pub fn epmng_mut(&self) -> RefMut<'_, EpMng> {
        self.epmng.borrow_mut()
    }

    /// Returns a reference to the activity's resource manager.
    pub fn resmng(&self) -> Option<&ResMng> {
        self.rmng.as_ref()
    }

    /// Returns a reference to the activity's pager.
    pub fn pager(&self) -> Option<&Pager> {
        self.pager.as_ref()
    }

    /// Allocates a new capability selector and returns it.
    pub fn alloc_sel(&self) -> Selector {
        self.alloc_sels(1)
    }

    /// Allocates `count` new and contiguous capability selectors and returns the first one.
    pub fn alloc_sels(&self, count: u64) -> Selector {
        let next = self.next_sel.get();
        self.next_sel.set(next + count);
        next
    }
}

impl Deref for OwnActivity {
    type Target = Activity;

    fn deref(&self) -> &<Self as Deref>::Target {
        &self.base
    }
}

impl fmt::Debug for OwnActivity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "OwnActivity[sel: {}, tile: {:?}]",
            self.sel(),
            self.tile()
        )
    }
}
