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

#[macro_export]
macro_rules! read_csr {
    ($reg_name:tt) => {{
        let res: usize;
        unsafe {
            asm!(
                concat!("csrr {0}, ", $reg_name),
                out(reg) res,
                options(nomem, nostack)
            )
        };
        res
    }}
}

#[macro_export]
macro_rules! write_csr {
    ($reg_name:tt, $val:expr) => {{
        unsafe {
            let val = $val;
            asm!(
                concat!("csrw ", $reg_name, ", {0}"),
                in(reg) val,
                options(nomem, nostack)
            )
        };
    }};
}

#[macro_export]
macro_rules! set_csr_bits {
    ($reg_name:tt, $bits:expr) => {{
        unsafe {
            let bits = $bits;
            asm!(
                concat!("csrs ", $reg_name, ", {0}"),
                in(reg) bits,
                options(nomem, nostack)
            )
        };
    }};
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn read8b(addr: usize) -> u64 {
    let res: u64;
    asm!(
        "ld {0}, ({1})",
        out(reg) res,
        in(reg) addr,
        options(nostack),
    );
    res
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn write8b(addr: usize, val: u64) {
    asm!(
        "sd {0}, ({1})",
        in(reg) val,
        in(reg) addr,
        options(nostack),
    )
}

#[inline(always)]
pub fn stack_pointer() -> usize {
    let sp: usize;
    unsafe {
        asm!(
            "mv {0}, sp",
            out(reg) sp,
            options(nomem, nostack),
        )
    }
    sp
}

#[inline(always)]
pub fn base_pointer() -> usize {
    let fp: usize;
    unsafe {
        asm!(
            "mv {0}, fp",
            out(reg) fp,
            options(nomem, nostack),
        )
    }
    fp
}

pub fn elapsed_cycles() -> u64 {
    let mut res: u64;
    unsafe {
        asm!(
            "rdcycle {0}",
            out(reg) res,
            options(nomem, nostack),
        );
    }
    res
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn backtrace_step(bp: usize, func: &mut usize) -> usize {
    let bp_ptr = bp as *const usize;
    *func = *bp_ptr.offset(-1);
    *bp_ptr.offset(-2)
}
