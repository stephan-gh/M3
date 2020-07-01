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

use time;

#[allow(clippy::missing_safety_doc)]
pub unsafe fn read8b(addr: usize) -> u64 {
    let res: u64;
    llvm_asm!(
        "ldrd $0, [$1]"
        : "=r"(res)
        : "r"(addr)
        : : "volatile"
    );
    res
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn write8b(addr: usize, val: u64) {
    llvm_asm!(
        "strd $0, [$1]"
        : : "r"(val), "r"(addr)
        : : "volatile"
    );
}

#[inline(always)]
pub fn get_sp() -> usize {
    let res: usize;
    unsafe {
        llvm_asm!(
            "mov $0, r13;"
            : "=r"(res)
        );
    }
    res
}

#[inline(always)]
pub fn get_bp() -> usize {
    let val: usize;
    unsafe {
        llvm_asm!(
            "mov $0, r11;"
            : "=r"(val)
        );
    }
    val
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn backtrace_step(bp: usize, func: &mut usize) -> usize {
    let bp_ptr = bp as *const usize;
    *func = *bp_ptr.offset(1);
    *bp_ptr
}

pub fn rdtsc() -> time::Time {
    // TODO implement me
    0
}

pub fn gem5_debug(msg: usize) -> time::Time {
    let mut res = msg as time::Time;
    unsafe {
        llvm_asm!(
            ".long 0xEE630110"
            : "+{r0}"(res)
            : : : "volatile"
        );
    }
    res
}
