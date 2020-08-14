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

use crate::time;

macro_rules! impl_read_reg {
    ($func_name:tt, $reg_name:tt) => {
        #[inline(always)]
        pub fn $func_name() -> usize {
            let res: usize;
            unsafe { llvm_asm!(concat!("mov %", $reg_name, ", $0") : "=r"(res)) };
            res
        }
    };
}

macro_rules! impl_write_reg {
    ($func_name:tt, $reg_name:tt) => {
        #[inline(always)]
        pub fn $func_name(val: usize) {
            unsafe { llvm_asm!(concat!("mov $0, %", $reg_name) : : "r"(val) : : "volatile") };
        }
    };
}

impl_read_reg!(read_cr0, "cr0");
impl_read_reg!(read_cr2, "cr2");
impl_read_reg!(read_cr3, "cr3");
impl_read_reg!(read_cr4, "cr4");

impl_write_reg!(write_cr0, "cr0");
impl_write_reg!(write_cr3, "cr3");
impl_write_reg!(write_cr4, "cr4");

impl_read_reg!(get_sp, "rsp");
impl_read_reg!(get_bp, "rbp");

#[allow(clippy::missing_safety_doc)]
pub unsafe fn read8b(addr: usize) -> u64 {
    let res: u64;
    llvm_asm!(
        "mov ($1), $0"
        : "=r"(res)
        : "r"(addr)
        : : "volatile"
    );
    res
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn write8b(addr: usize, val: u64) {
    llvm_asm!(
        "mov $0, ($1)"
        : : "r"(val), "r"(addr)
        : : "volatile"
    );
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn backtrace_step(bp: usize, func: &mut usize) -> usize {
    let bp_ptr = bp as *const usize;
    *func = *bp_ptr.offset(1);
    *bp_ptr
}

pub fn rdtsc() -> time::Time {
    let u: u32;
    let l: u32;
    unsafe {
        llvm_asm!(
            "rdtsc"
            : "={rax}"(l), "={rdx}"(u)
        );
    }
    time::Time::from(u) << 32 | time::Time::from(l)
}

pub fn gem5_debug(msg: usize) -> time::Time {
    let res: time::Time;
    unsafe {
        llvm_asm!(
            ".byte 0x0F, 0x04;
             .word 0x63"
            : "={rax}"(res)
            : "{rdi}"(msg)
            : : "volatile"
        );
    }
    res
}
