/*
 * Copyright (C) 2020-2022 Nils Asmussen, Barkhausen Institut
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

use crate::arch::envdata;
use crate::arch::tcu;
use crate::errors::Error;

extern "C" {
    pub fn gem5_writefile(src: *const u8, len: u64, offset: u64, file: u64);
}

pub fn write(buf: &[u8]) -> Result<usize, Error> {
    let amount = tcu::TCU::print(buf);
    if envdata::get().platform == crate::envdata::Platform::GEM5.val {
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

pub fn init() {
}
