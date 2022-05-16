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

use core::arch::asm;

macro_rules! impl_read_reg {
    ($func_name:tt, $reg_name:tt) => {
        #[inline(always)]
        pub fn $func_name() -> usize {
            let res: usize;
            unsafe {
                asm!(
                    concat!("mov {0}, ", $reg_name),
                    out(reg) res,
                    options(nostack, nomem),
                )
            };
            res
        }
    };
}

macro_rules! impl_write_reg {
    ($func_name:tt, $reg_name:tt) => {
        #[inline(always)]
        pub fn $func_name(val: usize) {
            unsafe {
                asm!(
                    concat!("mov ", $reg_name, ", {0}"),
                    in(reg) val,
                    options(nostack, nomem),
                )
            };
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

impl_read_reg!(stack_pointer, "rsp");
impl_read_reg!(base_pointer, "rbp");

#[allow(clippy::missing_safety_doc)]
pub unsafe fn read8b(addr: usize) -> u64 {
    let res: u64;
    asm!(
        "mov {0}, [{1}]",
        out(reg) res,
        in(reg) addr,
        options(nostack),
    );
    res
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn write8b(addr: usize, val: u64) {
    asm!(
        "mov [{1}], {0}",
        in(reg) val,
        in(reg) addr,
        options(nostack),
    );
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn backtrace_step(bp: usize, func: &mut usize) -> usize {
    let bp_ptr = bp as *const usize;
    *func = *bp_ptr.offset(1);
    *bp_ptr
}

pub fn elapsed_cycles() -> u64 {
    let u: u32;
    let l: u32;
    unsafe {
        asm!(
            "rdtsc",
            out("rax") l,
            out("rdx") u,
            options(nostack, nomem),
        );
    }
    u64::from(u) << 32 | u64::from(l)
}

pub fn gem5_debug(msg: u64) -> u64 {
    let res: u64;
    unsafe {
        asm!(
            ".byte 0x0F, 0x04",
            ".word 0x63",
            out("rax") res,
            in("rdi") msg,
            options(nostack),
        );
    }
    res
}
