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

use arch::envdata;
use arch::tcu;
use cfg;
use core::ptr;
use errors::Error;
use libc;

extern "C" {
    pub fn gem5_writefile(src: *const u8, len: u64, offset: u64, file: u64);
    pub fn gem5_readfile(dst: *mut u8, max: u64, offset: u64) -> i64;
}

pub fn read(buf: &mut [u8]) -> Result<usize, Error> {
    if envdata::get().platform == envdata::Platform::GEM5.val {
        unsafe { Ok(gem5_readfile(buf.as_mut_ptr(), buf.len() as u64, 0) as usize) }
    }
    else {
        unimplemented!();
    }
}

pub fn write(buf: &[u8]) -> Result<usize, Error> {
    if envdata::get().platform == envdata::Platform::GEM5.val {
        tcu::TCU::print(buf);
        unsafe {
            // put the string on the stack to prevent that gem5_writefile causes a pagefault
            let file: [u8; 7] = *b"stdout\0";
            gem5_writefile(buf.as_ptr(), buf.len() as u64, 0, file.as_ptr() as u64);
        }
    }
    else {
        let signal = cfg::SERIAL_SIGNAL as *mut u64;
        let serbuf = cfg::SERIAL_BUF as *mut i8;
        unsafe {
            libc::memcpy(
                serbuf as *mut libc::c_void,
                buf.as_ptr() as *const libc::c_void,
                buf.len(),
            );
            *serbuf.offset(buf.len() as isize) = 0;
            *signal = buf.len() as u64;
            while ptr::read_volatile(signal) != 0 {}
        }
    }
    Ok(buf.len())
}

pub fn init() {
}
