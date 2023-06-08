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

use crate::arch::CPUOps;
use crate::mem::VirtAddr;

pub struct ARMCPU {}

impl CPUOps for ARMCPU {
    unsafe fn read8b(addr: *const u64) -> u64 {
        // dual registers are unfortunately no longer supported with the new asm! macro. thus, we work
        // around that by hardcoding the registers here.
        let lo: u32;
        let hi: u32;
        asm! {
            "ldrd r2, r3, [{0}]",
            in(reg) addr as usize,
            lateout("r2") lo,
            out("r3") hi,
            options(nostack),
        }
        ((hi as u64) << 32) | (lo as u64)
    }

    unsafe fn write8b(addr: *mut u64, val: u64) {
        // see `read8b`
        let lo = val as u32;
        let hi = (val >> 32) as u32;
        asm! {
            "strd r2, r3, [{0}]",
            in(reg) addr as usize,
            in("r2") lo,
            in("r3") hi,
            options(nostack),
        }
    }

    #[inline(always)]
    fn stack_pointer() -> VirtAddr {
        let sp: usize;
        unsafe {
            asm!(
                "mov {0}, r13",
                out(reg) sp,
                options(nomem, nostack),
            )
        }
        VirtAddr::from(sp)
    }

    #[inline(always)]
    fn base_pointer() -> VirtAddr {
        let fp: usize;
        unsafe {
            asm!(
                "mov {0}, r11",
                out(reg) fp,
                options(nomem, nostack),
            )
        }
        VirtAddr::from(fp)
    }

    unsafe fn backtrace_step(bp: VirtAddr, func: &mut VirtAddr) -> VirtAddr {
        let bp_ptr = bp.as_ptr::<usize>();
        *func = VirtAddr::from(*bp_ptr.offset(1));
        VirtAddr::from(*bp_ptr)
    }

    fn elapsed_cycles() -> u64 {
        // TODO for now we use our custom instruction
        Self::gem5_debug(0)
    }

    fn gem5_debug(msg: u64) -> u64 {
        // see `read8b`
        let mut lo = (msg & 0xFFFF_FFFF) as u32;
        let mut hi = (msg >> 32) as u32;
        unsafe {
            asm!(
                ".long 0xEE630110",
                inout("r0") lo,
                inout("r1") hi,
                options(nostack),
            );
        }
        ((hi as u64) << 32) | (lo as u64)
    }
}
