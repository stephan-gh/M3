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

mod activity;
mod childactivity;
mod kmem;
mod mapper;
mod ownactivity;
mod running;
mod state;
mod tile;

pub use self::activity::Activity;
pub use self::childactivity::{ActivityArgs, ChildActivity};
pub use self::kmem::KMem;
pub use self::mapper::{DefaultMapper, Mapper};
pub use self::ownactivity::OwnActivity;
pub use self::running::{RunningActivity, RunningDeviceActivity, RunningProgramActivity};
pub use self::state::{StateDeserializer, StateSerializer};
pub use self::tile::{Tile, TileQuota};

pub(crate) fn init() {
    self::activity::init();
}
