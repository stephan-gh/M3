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

use base::pexif;

use crate::arch::pexabi;
use crate::errors::Error;
use crate::tcu::{EpId, INVALID_EP};

#[inline(always)]
pub fn sleep(nanos: u64, ep: Option<EpId>) -> Result<(), Error> {
    pexabi::call2(
        pexif::Operation::SLEEP,
        nanos as usize,
        ep.unwrap_or(INVALID_EP) as usize,
    )
    .map(|_| ())
}

pub fn exit(code: i32) -> ! {
    pexabi::call1(pexif::Operation::EXIT, code as usize).ok();
    unreachable!();
}

pub fn flush_invalidate() -> Result<(), Error> {
    pexabi::call1(pexif::Operation::FLUSH_INV, 0).map(|_| ())
}

#[inline(always)]
pub fn switch_vpe() -> Result<(), Error> {
    pexabi::call1(pexif::Operation::YIELD, 0).map(|_| ())
}

#[inline(always)]
pub fn noop() -> Result<(), Error> {
    pexabi::call1(pexif::Operation::NOOP, 0).map(|_| ())
}
