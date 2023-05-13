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

//! Contains tile-related abstractions
//!
//! M³ builds upon a tiled hardware architecture consisting of memory and compute tiles. This module
//! deals with the allocation of compute tiles and the execution of *activities* on these tiles.
//!
//! # Tile allocation and derivation
//!
//! Provided that an application has the required permissions, it can allocate tiles using the
//! [`Tile`] abstraction. Allocating a tile provides this application full access to the tile's
//! inner resources. However, the TCU remains under the control of the M³ kernel and thus every
//! access to tile-external resources is restricted. Furthermore, the resources of the tile (CPU
//! time, endpoints, etc.) can be split by *deriving* a new [`Tile`] object from an existing one.
//! Since the creation of child activities (see below) requires a tile capability, different
//! activities on the same tile can be run with different resource shares.
//!
//! # Activities
//!
//! An allocated tile allows to execute activities on the tile, represented by [`Activity`]. On
//! general-purpose tiles, the activity executes code on the core. On accelerator/device tiles, the
//! activity uses the logic of the accelerator/device.
//!
//! The own activity is represented by [`OwnActivity`], whereas created activities are represented
//! by [`ChildActivity`]. The former provides access to resources associated with the own activity
//! such as the [`Pager`](`crate::client::Pager`), the [`EpMng`](`crate::com::EpMng`), and the
//! [`ResMng`](`crate::client::ResMng`). After creation of a [`ChildActivity`], it is first
//! configured accordingly (delegating capabilities, files, mount points, and data to the child) and
//! finally started, which yields a [`RunningActivity`].

mod activity;
mod childactivity;
mod kmem;
mod loader;
mod mapper;
mod ownactivity;
mod running;
mod tile;

pub use self::activity::Activity;
pub use self::childactivity::{ActivityArgs, ChildActivity};
pub use self::kmem::KMem;
pub use self::mapper::{DefaultMapper, Mapper};
pub use self::ownactivity::OwnActivity;
pub use self::running::{RunningActivity, RunningDeviceActivity, RunningProgramActivity};
pub use self::tile::{Tile, TileArgs, TileQuota};

pub(crate) fn init() {
    self::activity::init();
}
