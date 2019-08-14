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

use arch::pexcalls::Operation;
use errors::Error;

fn get_result(res: isize) -> Result<usize, Error> {
    match res {
        e if e < 0 => Err(Error::from(-e as u32)),
        val => Ok(val as usize),
    }
}

pub fn call1(op: Operation, arg1: usize) -> Result<usize, Error> {
    let mut res = op.val;
    unsafe {
        asm!(
            "svc $$0"
            : "+{r0}"(res)
            : "{r1}"(arg1)
            : "memory"
        );
    }
    get_result(res)
}

pub fn call2(op: Operation, arg1: usize, arg2: usize) -> Result<usize, Error> {
    let mut res = op.val;
    unsafe {
        asm!(
            "svc $$63"
            : "+{r0}"(res)
            : "{r1}"(arg1), "{r2}"(arg2)
            : "memory"
        );
    }
    get_result(res)
}

pub fn call3(op: Operation, arg1: usize, arg2: usize, arg3: usize) -> Result<usize, Error> {
    let mut res = op.val;
    unsafe {
        asm!(
            "svc $$63"
            : "+{r0}"(res)
            : "{r1}"(arg1), "{r2}"(arg2), "{r3}"(arg3)
            : "memory"
        );
    }
    get_result(res)
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
            "svc $$63"
            : "+{r0}"(res)
            : "{r1}"(arg1), "{r2}"(arg2), "{r3}"(arg3), "{r4}"(arg4)
            : "memory"
        );
    }
    get_result(res)
}

pub fn call5(
    op: Operation,
    arg1: usize,
    arg2: usize,
    arg3: usize,
    arg4: usize,
    arg5: usize,
) -> Result<usize, Error> {
    let mut res = op.val;
    unsafe {
        asm!(
            "svc $$63"
            : "+{r0}"(res)
            : "{r1}"(arg1), "{r2}"(arg2), "{r3}"(arg3), "{r4}"(arg4), "{r5}"(arg5)
            : "memory"
        );
    }
    get_result(res)
}
