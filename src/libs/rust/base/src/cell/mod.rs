/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2021 Nils Asmussen, Barkhausen Institut
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

//! Shareable mutable containers

mod lazy;
mod stat;
mod statref;
mod statunsafe;

pub use self::lazy::{LazyReadOnlyCell, LazyStaticCell, LazyStaticRefCell};
pub use self::stat::StaticCell;
pub use self::statref::StaticRefCell;
pub use self::statunsafe::{LazyStaticUnsafeCell, StaticUnsafeCell};
pub use core::cell::{Cell, Ref, RefCell, RefMut, UnsafeCell};
