/*
 * Copyright (C) 2021 Mark Ueberall <mark.ueberall.1999@gmail.com>
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

use core::fmt::Display;

use crate::errors::{Code, Error};

impl serde::ser::Error for Error {
    fn custom<T: Display>(_msg: T) -> Self {
        // TODO use/pass-on the message somehow
        Error::new(Code::InvArgs)
    }
}

impl serde::de::Error for Error {
    fn custom<T: Display>(_msg: T) -> Self {
        // TODO use/pass-on the message somehow
        Error::new(Code::InvArgs)
    }
}
