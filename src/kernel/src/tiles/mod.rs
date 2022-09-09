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

mod activities;
mod actmng;
pub mod loader;
pub mod tilemng;
mod tilemux;

pub use self::activities::{Activity, ActivityFlags, State, INVAL_ID, KERNEL_ID};
pub use self::actmng::ActivityMng;
pub use self::tilemux::TileMux;

pub fn init() {
    self::tilemng::init();
    self::actmng::init();
}

pub fn deinit() {
    self::actmng::deinit();
}
