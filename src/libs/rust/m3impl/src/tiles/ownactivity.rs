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
use core::ops::Deref;

use crate::cap::{CapFlags, Capability};
use crate::cell::{Ref, RefCell, RefMut};
use crate::client::ResMng;
use crate::com::EpMng;
use crate::env;
use crate::errors::{Code, Error};
use crate::kif;
use crate::rc::Rc;
use crate::serialize::M3Deserializer;
use crate::tcu::{EpId, INVALID_EP, TCU};
use crate::tiles::{Activity, KMem, Tile};
use crate::time::TimeDuration;
use crate::tmif;
use crate::vfs::{FileTable, MountTable};

/// Represents the own activity
///
/// The own activity provides access to the resources associated with this activity, such as the
/// pager, the resource manager, and endpoints. Additionally, it provides access to the resources
/// that can be transferred to [`ChildActivity`](`crate::tiles::ChildActivity`)s: files, mount
/// points, and data.
///
/// Besides access to these resources, [`OwnActivity`] offers operations regarding the execution of
/// the own activity such as [`sleep`](`OwnActivity::sleep`), [`wait_for`](`OwnActivity::wait_for`),
/// and [`exit`](`OwnActivity::exit`).
pub struct OwnActivity {
    base: Activity,
    epmng: RefCell<EpMng>,
    files: RefCell<FileTable>,
    mounts: RefCell<MountTable>,
}

impl OwnActivity {
    pub(crate) fn new() -> Self {
        let env = crate::env::get();
        OwnActivity {
            base: Activity {
                id: env.activity_id(),
                cap: Capability::new(kif::SEL_ACT, CapFlags::KEEP_CAP),
                tile: Rc::new(Tile::new_bind(
                    env.tile_id(),
                    env.tile_desc(),
                    kif::SEL_TILE,
                )),
                eps_start: env.first_std_ep(),
                rmng: env.load_rmng(),
                pager: env.load_pager(),
                data: env.load_data(),
                kmem: Rc::new(KMem::new(kif::SEL_KMEM)),
            },
            epmng: RefCell::new(EpMng::default()),
            // mounts first; files depend on mounts
            mounts: RefCell::new(env.load_mounts()),
            files: RefCell::new(env.load_fds()),
        }
    }

    /// Exits with an unspecified error without deinitialization
    pub fn abort() -> ! {
        base::machine::write_coverage(env::get().activity_id() as u64 + 1);
        tmif::exit(Code::Unspecified);
    }

    // Deinitializes all data structures and exits with given result
    pub fn exit(res: Result<(), Error>) -> ! {
        let err = match res {
            Ok(_) => Code::Success,
            Err(e) => e.code(),
        };
        Self::exit_with(err);
    }

    // Deinitializes all data structures and exits with given error
    pub fn exit_with(err: Code) -> ! {
        crate::env::deinit();
        base::machine::write_coverage(env::get().activity_id() as u64 + 1);
        tmif::exit(err);
    }

    /// Puts the own activity to sleep until the next message arrives
    #[inline(always)]
    pub fn sleep() -> Result<(), Error> {
        Self::sleep_for(TimeDuration::MAX)
    }

    /// Puts the own activity to sleep until the next message arrives or `timeout` time has passed.
    #[inline(always)]
    pub fn sleep_for(timeout: TimeDuration) -> Result<(), Error> {
        if crate::env::get().shared() || timeout != TimeDuration::MAX {
            let timeout = match timeout {
                TimeDuration::MAX => None,
                t => Some(t),
            };
            return tmif::wait(None, None, timeout);
        }
        if crate::env::get().platform() != crate::env::Platform::Hw {
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
        ep: Option<EpId>,
        irq: Option<tmif::IRQId>,
        timeout: Option<TimeDuration>,
    ) -> Result<(), Error> {
        if crate::env::get().shared() {
            return tmif::wait(ep, irq, timeout);
        }
        if crate::env::get().platform() != crate::env::Platform::Hw {
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
    pub fn data_source(&self) -> M3Deserializer<'_> {
        M3Deserializer::new(&self.data)
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
