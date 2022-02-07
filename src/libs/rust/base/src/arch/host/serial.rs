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

use alloc::format;
use libc;

use crate::envdata;
use crate::errors::{Code, Error};

static mut LOG_FD: i32 = -1;

pub fn write(buf: &[u8]) -> Result<usize, Error> {
    match unsafe { libc::write(LOG_FD, buf.as_ptr() as *const libc::c_void, buf.len()) } {
        res if res < 0 => Err(Error::new(Code::WriteFailed)),
        res => Ok(res as usize),
    }
}

pub fn init() {
    unsafe {
        let path = format!("{}/log.txt\0", envdata::out_dir());
        LOG_FD = libc::open(
            path.as_ptr() as *const libc::c_char,
            libc::O_WRONLY | libc::O_APPEND,
        );
        assert!(LOG_FD != -1);
    }
}
