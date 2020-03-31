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

use m3::com::MemGate;
use m3::errors::Error;

pub trait BlockDeviceTrait {
    fn partition_exists(&self, part: usize) -> bool;

    fn read(
        &mut self,
        part: usize,
        buf: &MemGate,
        buf_off: usize,
        disk_off: usize,
        bytes: usize,
    ) -> Result<(), Error>;

    fn write(
        &mut self,
        part: usize,
        buf: &MemGate,
        buf_off: usize,
        disk_off: usize,
        bytes: usize,
    ) -> Result<(), Error>;
}

#[cfg(target_os = "linux")]
#[path = "host/mod.rs"]
mod backend_impl;

#[cfg(target_os = "none")]
#[path = "gem5/mod.rs"]
mod backend_impl;

pub use self::backend_impl::BlockDevice;
