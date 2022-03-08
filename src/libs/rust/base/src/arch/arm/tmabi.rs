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

use core::arch::asm;

use crate::errors::Error;
use crate::tmif::Operation;

pub fn call1(op: Operation, arg1: usize) -> Result<usize, Error> {
    call2(op, arg1, 0)
}

pub fn call2(op: Operation, arg1: usize, arg2: usize) -> Result<usize, Error> {
    let mut res = op.val;
    unsafe {
        asm!(
            // XXX: hack to make these calls work for the kernel. the problem is that the kernel
            // runs in supervisor mode, but the ISR code assumes that the interrupted code runs in
            // user mode and therefore saves the userspace lr. to work around that, we simply save
            // and restore lr before and after the call. since we don't care about ARM too much,
            // this seems fine for now.
            "mov r5, lr",
            "svc 0",
            "mov lr, r5",
            inout("r0") res,
            in("r1") arg1,
            in("r2") arg2,
            // mark r5 as clobbered
            out("r5") _,
        );
    }
    crate::tmif::get_result(res)
}

pub fn call3(op: Operation, arg1: usize, arg2: usize, arg3: usize) -> Result<usize, Error> {
    let mut res = op.val;
    unsafe {
        asm!(
            // see above
            "mov r5, lr",
            "svc 0",
            "mov lr, r5",
            inout("r0") res,
            in("r1") arg1,
            in("r2") arg2,
            in("r3") arg3,
            out("r5") _,
        );
    }
    crate::tmif::get_result(res)
}

pub fn call4(
    op: Operation,
    arg1: usize,
    arg2: usize,
    arg3: usize,
    arg4: usize,
) -> Result<usize, Error> {
    let mut res = op.val;
    unsafe {
        asm!(
            // see above
            "mov r5, lr",
            "svc 0",
            "mov lr, r5",
            inout("r0") res,
            in("r1") arg1,
            in("r2") arg2,
            in("r3") arg3,
            in("r4") arg4,
            out("r5") _,
        );
    }
    crate::tmif::get_result(res)
}
