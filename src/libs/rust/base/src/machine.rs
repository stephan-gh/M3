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

//! Machine-specific functions

use crate::env;
use crate::errors::Error;
use crate::tcu;

extern "C" {
    pub fn gem5_writefile(src: *const u8, len: u64, offset: u64, file: u64);
    pub fn gem5_shutdown(delay: u64);
}

pub fn write_coverage(act: u64) {
    if env::data().platform == env::Platform::GEM5.val {
        let coverage = minicov::capture_coverage();
        tcu::TCU::write_coverage(&coverage, act);
    }
}

pub fn write(buf: &[u8]) -> Result<usize, Error> {
    let amount = tcu::TCU::print(buf);
    if env::data().platform == env::Platform::GEM5.val {
        unsafe {
            // put the string on the stack to prevent that gem5_writefile causes a pagefault
            let file: [u8; 7] = *b"stdout\0";
            // touch the string first to cause a page fault, if required. gem5 assumes that it's mapped
            let _b = file.as_ptr().read_volatile();
            let _b = file.as_ptr().add(6).read_volatile();
            gem5_writefile(buf.as_ptr(), amount as u64, 0, file.as_ptr() as u64);
        }
    }
    Ok(amount)
}

pub fn shutdown() -> ! {
    if env::data().platform == env::Platform::GEM5.val {
        unsafe {
            gem5_shutdown(0)
        };
    }
    else {
        #[cfg(target_arch = "riscv64")]
        unsafe {
            core::arch::asm!("1: j 1b")
        };
    }
    unreachable!();
}
