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

use base::pexif::Operation;
use errors::Error;

pub fn call1(op: Operation, arg1: usize) -> Result<usize, Error> {
    call2(op, arg1, 0)
}

#[cfg(target_os = "none")]
pub fn call2(op: Operation, arg1: usize, arg2: usize) -> Result<usize, Error> {
    let mut res = op.val;
    unsafe {
        asm!(
            "int $$63"
            : "+{rax}"(res)
            : "{rcx}"(arg1), "{rdx}"(arg2)
            : "memory"
        );
    }
    crate::arch::get_result(res)
}

#[cfg(target_os = "linux")]
pub fn call2(_op: Operation, _arg1: usize, _arg2: usize) -> Result<usize, Error> {
    Ok(0)
}
