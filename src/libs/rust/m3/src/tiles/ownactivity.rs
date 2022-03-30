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
use core::ops::{Deref, DerefMut};

use crate::arch;
use crate::cap::{CapFlags, Capability, Selector};
use crate::com::EpMng;
use crate::errors::Error;
use crate::kif;
use crate::rc::Rc;
use crate::session::{Pager, ResMng};
use crate::tcu::{EpId, INVALID_EP, TCU};
use crate::tiles::{Activity, KMem, StateDeserializer, Tile};
use crate::time::TimeDuration;
use crate::tmif;
use crate::vfs::{FileTable, MountTable};

/// Represents the own activity.
pub struct OwnActivity {
    base: Activity,
    epmng: EpMng,
    files: FileTable,
    mounts: MountTable,
}

impl OwnActivity {
    pub(crate) fn new_cur() -> Self {
        OwnActivity {
            base: Activity::new_act(
                Capability::new(kif::SEL_ACT, CapFlags::KEEP_CAP),
                Rc::new(Tile::new_bind(0, kif::TileDesc::new_from(0), kif::SEL_TILE)),
                Rc::new(KMem::new(kif::SEL_KMEM)),
            ),
            epmng: EpMng::default(),
            files: FileTable::default(),
            mounts: MountTable::default(),
        }
    }

    pub(crate) fn init_cur(&mut self) {
        self.base.init_act();
        let env = arch::env::get();
        // mounts first; files depend on mounts
        self.mounts = env.load_mounts();
        self.files = env.load_fds();
        self.epmng.reset();
    }

    /// Puts the current activity to sleep until the next message arrives
    #[inline(always)]
    pub fn sleep(&self) -> Result<(), Error> {
        self.sleep_for(TimeDuration::MAX)
    }

    /// Puts the current activity to sleep until the next message arrives or `timeout` time has passed.
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

    /// Puts the current activity to sleep until the next message arrives on the given EP
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
    pub fn files(&mut self) -> &mut FileTable {
        &mut self.files
    }

    /// Returns a mutable reference to the mount table of this activity.
    pub fn mounts(&mut self) -> &mut MountTable {
        &mut self.mounts
    }

    /// Returns a source for the activity-local data
    ///
    /// The source provides access to the activity-local data that has been transmitted to this
    /// activity from its parent during [`run`](Activity::run) and [`exec`](Activity::exec).
    pub fn data_source(&self) -> StateDeserializer<'_> {
        StateDeserializer::new(&self.data)
    }

    /// Returns a reference to the endpoint manager
    pub fn epmng(&self) -> &EpMng {
        &self.epmng
    }

    /// Returns a mutable reference to the endpoint manager
    pub fn epmng_mut(&mut self) -> &mut EpMng {
        &mut self.epmng
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
    pub fn alloc_sel(&mut self) -> Selector {
        self.alloc_sels(1)
    }

    /// Allocates `count` new and contiguous capability selectors and returns the first one.
    pub fn alloc_sels(&mut self, count: u64) -> Selector {
        self.next_sel += count;
        self.next_sel - count
    }
}

impl Deref for OwnActivity {
    type Target = Activity;

    fn deref(&self) -> &<Self as Deref>::Target {
        &self.base
    }
}

impl DerefMut for OwnActivity {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
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
