/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

use crate::arch;
use crate::com;
use crate::io;
use crate::syscalls;
use crate::tiles;
use crate::tmif;
use crate::vfs;

use core::ptr;

#[no_mangle]
pub extern "C" fn abort() -> ! {
    tmif::exit(1);
}

#[no_mangle]
pub extern "C" fn exit(_code: i32) -> ! {
    io::deinit();
    vfs::deinit();

    tmif::exit(_code);
}

extern "C" {
    fn __m3_init_libc(argc: i32, argv: *const *const u8, envp: *const *const u8);
    fn main() -> i32;
}

#[no_mangle]
pub extern "C" fn env_run() {
    unsafe {
        __m3_init_libc(0, ptr::null(), ptr::null());
    }
    syscalls::init();
    com::pre_init();
    tiles::init();
    io::init();
    com::init();

    let res = if let Some(cl) = arch::env::get().load_closure() {
        cl()
    }
    else {
        unsafe { main() }
    };

    exit(res)
}
