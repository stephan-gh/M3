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

//! Contains the serial struct

use core::fmt;

use crate::arch;
use crate::errors::Error;
use crate::io;

/// The serial line
pub struct Serial {}

impl Serial {
    pub const fn new() -> Self {
        Self {}
    }
}

impl io::Read for Serial {
    fn read(&mut self, _buf: &mut [u8]) -> Result<usize, Error> {
        // there is never anything to read
        Ok(0)
    }
}

impl io::Write for Serial {
    fn write(&mut self, mut buf: &[u8]) -> Result<usize, Error> {
        let res = buf.len();
        while !buf.is_empty() {
            match arch::serial::write(buf) {
                Err(e) => return Err(e),
                Ok(n) => buf = &buf[n..],
            }
        }
        Ok(res)
    }
}

impl fmt::Debug for Serial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Serial")
    }
}
