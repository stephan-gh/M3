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

pub static DEF: bool = true;
pub static ERR: bool = true;
pub static EPS: bool = false;
pub static SYSC: bool = false;
pub static CAPS: bool = false;
pub static MEM: bool = false;
pub static KMEM: bool = false;
pub static SERV: bool = false;
pub static SQUEUE: bool = false;
pub static ACTIVITIES: bool = false;
pub static TMC: bool = false;
pub static TILES: bool = false;
pub static UPCALLS: bool = false;
pub static SLAB: bool = false;
pub static KTCU: bool = false;

#[macro_export]
macro_rules! klog {
    ($type:tt, $fmt:expr)              => (
        base::llog!(@log_impl $crate::log::$type, concat!($fmt, "\n"))
    );
    ($type:tt, $fmt:expr, $($arg:tt)*) => (
        base::llog!(@log_impl $crate::log::$type, concat!($fmt, "\n"), $($arg)*)
    );
}
