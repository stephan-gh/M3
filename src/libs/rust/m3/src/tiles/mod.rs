/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
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
mod kmem;
mod mapper;
mod running;
mod state;
mod tile;

pub use self::activity::{Activity, ActivityArgs};
pub use self::kmem::KMem;
pub use self::mapper::{DefaultMapper, Mapper};
pub use self::running::{RunningActivity, RunningDeviceActivity, RunningProgramActivity};
pub use self::state::{StateDeserializer, StateSerializer};
pub use self::tile::Tile;

pub(crate) fn init() {
    self::activity::init();
}
