/*
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

use crate::errors::Error;
use crate::tmif::Operation;

pub fn call1(op: Operation, arg1: usize) -> Result<usize, Error> {
    call2(op, arg1, 0)
}

#[cfg(not(target_vendor = "host"))]
pub fn call2(op: Operation, arg1: usize, arg2: usize) -> Result<usize, Error> {
    let mut res = op.val;
    unsafe {
        core::arch::asm!(
            "int $63",
            inout("rax") res,
            in("rcx") arg1,
            in("rdx") arg2,
        );
    }
    crate::tmif::get_result(res)
}

#[cfg(target_vendor = "host")]
pub fn call2(_op: Operation, _arg1: usize, _arg2: usize) -> Result<usize, Error> {
    Ok(0)
}

#[cfg(not(target_vendor = "host"))]
pub fn call3(op: Operation, arg1: usize, arg2: usize, arg3: usize) -> Result<usize, Error> {
    let mut res = op.val;
    unsafe {
        core::arch::asm!(
            "int $63",
            inout("rax") res,
            in("rcx") arg1,
            in("rdx") arg2,
            in("rdi") arg3,
        );
    }
    crate::tmif::get_result(res)
}

#[cfg(target_vendor = "host")]
pub fn call3(_op: Operation, _arg1: usize, _arg2: usize, _arg3: usize) -> Result<usize, Error> {
    Ok(0)
}

#[cfg(not(target_vendor = "host"))]
pub fn call4(
    op: Operation,
    arg1: usize,
    arg2: usize,
    arg3: usize,
    arg4: usize,
) -> Result<usize, Error> {
    let mut res = op.val;
    unsafe {
        core::arch::asm!(
            "int $63",
            inout("rax") res,
            in("rcx") arg1,
            in("rdx") arg2,
            in("rdi") arg3,
            in("rsi") arg4,
        );
    }
    crate::tmif::get_result(res)
}

#[cfg(target_vendor = "host")]
pub fn call4(
    _op: Operation,
    _arg1: usize,
    _arg2: usize,
    _arg3: usize,
    _arg4: usize,
) -> Result<usize, Error> {
    Ok(0)
}
