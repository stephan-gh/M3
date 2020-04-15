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

#[macro_export]
macro_rules! read_csr {
    ($reg_name:tt) => {{
        let res: usize;
        unsafe { asm!(concat!("csrr $0, ", $reg_name) : "=r"(res)) };
        res
    }}
}

#[macro_export]
macro_rules! write_csr {
    ($reg_name:tt, $val:expr) => {{
        unsafe { asm!(concat!("csrw ", $reg_name, ", $0") : : "r"($val) : : "volatile") };
    }};
}

#[macro_export]
macro_rules! set_csr_bits {
    ($reg_name:tt, $bits:expr) => {{
        unsafe { asm!(concat!("csrs ", $reg_name, ", $0") : : "r"($bits) : : "volatile") };
    }};
}

pub fn read8b(addr: usize) -> u64 {
    let res: u64;
    unsafe {
        asm!(
            "ld $0, ($1)"
            : "=r"(res)
            : "r"(addr)
            : : "volatile"
        );
    }
    res
}

pub fn write8b(addr: usize, val: u64) {
    unsafe {
        asm!(
            "sd $0, ($1)"
            : : "r"(val), "r"(addr)
        )
    }
}

#[inline(always)]
pub fn get_sp() -> usize {
    let sp: usize;
    unsafe {
        asm!(
            "mv $0, sp"
            : "=r"(sp)
        )
    }
    return sp;
}

#[inline(always)]
pub fn get_bp() -> usize {
    let fp: usize;
    unsafe {
        asm!(
            "mv $0, fp"
            : "=r"(fp)
        )
    }
    return fp;
}

pub unsafe fn backtrace_step(bp: usize, func: &mut usize) -> usize {
    let bp_ptr = bp as *const usize;
    *func = *bp_ptr.offset(-1);
    *bp_ptr.offset(-2)
}

pub fn gem5_debug(msg: usize) -> time::Time {
    let mut res = msg as time::Time;
    unsafe {
        asm!(
            ".long 0xC600007B"
            : "+{x10}"(res)
            : : : "volatile"
        );
    }
    res
}
