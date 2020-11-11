/*
 * Copyright (C) 2015-2020, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Copyright (C) 2019-2020, Tendsin Mende <tendsin@protonmail.com>
 * Copyright (C) 2018, Sebastian Reimers <sebastian.reimers@mailbox.tu-dresden.de>
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

use crate::data::BlockNo;

use m3::errors::Error;

pub const PRDT_SIZE: usize = 8;

/// Implemented by File and Meta buffer, defines shared behavior.
pub trait Buffer {
    type HEAD;

    fn mark_dirty(&mut self, bno: BlockNo);
    fn flush(&mut self) -> Result<(), Error>;

    fn get(&self, bno: BlockNo) -> Option<&Self::HEAD>;
    fn get_mut(&mut self, bno: BlockNo) -> Option<&mut Self::HEAD>;

    fn flush_chunk(head: &mut Self::HEAD) -> Result<(), Error>;
}
