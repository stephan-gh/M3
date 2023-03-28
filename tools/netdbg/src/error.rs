/*
 * Copyright (C) 2022 Nils Asmussen, Barkhausen Institut
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
    Io(io::Error),
    ParseNum(num::ParseIntError),
    NotFoundNIC(String),
    NotFoundNet(String),
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

impl_err!(io::Error, Io);
impl_err!(num::ParseIntError, ParseNum);

impl fmt::Debug for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            Error::Io(e) => write!(fmt, "I/O error occurred: {}", e),
            Error::ParseNum(e) => write!(fmt, "Unable to parse number: {}", e),
            Error::NotFoundNIC(e) => write!(fmt, "Unable to find NIC '{}'", e),
            Error::NotFoundNet(e) => write!(fmt, "Unable to find Net '{}'", e),
        }
    }
}
