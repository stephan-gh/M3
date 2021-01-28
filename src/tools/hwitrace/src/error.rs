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

use std::fmt;
use std::io;
use std::num;

pub enum Error {
    IoError(io::Error),
    NumError(num::ParseIntError),
    ObjdumpMalformed,
    ObjdumpError(i32),
    InvalPath,
}

macro_rules! impl_err {
    ($src:ty, $dst:tt) => {
        impl From<$src> for Error {
            fn from(error: $src) -> Self {
                Error::$dst(error)
            }
        }
    };
}

impl_err!(io::Error, IoError);
impl_err!(num::ParseIntError, NumError);

impl fmt::Debug for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            Error::IoError(e) => write!(fmt, "I/O error occurred: {}", e),
            Error::NumError(e) => write!(fmt, "Unable to parse number: {}", e),
            Error::ObjdumpMalformed => write!(fmt, "malformed objdump output"),
            Error::ObjdumpError(c) => write!(fmt, "objdump -SC <bin> failed: {}", c),
            Error::InvalPath => write!(fmt, "path is invalid"),
        }
    }
}
