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

//! Contains PE-related abstractions

mod activity;
mod kmem;
mod mapper;
mod pe;
mod vpe;

pub use self::activity::{Activity, ClosureActivity, ExecActivity};
pub use self::kmem::KMem;
pub use self::mapper::{Mapper, DefaultMapper};
pub use self::pe::PE;
pub use self::vpe::{VPE, VPEArgs};

pub(crate) fn init() {
    self::vpe::init();
}

pub(crate) fn reinit() {
    self::vpe::reinit();
}
