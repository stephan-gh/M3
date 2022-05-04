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
use crate::kif;
use crate::libc;
use crate::syscalls;
use crate::tiles;
use crate::vfs;

pub fn exit(code: i32) -> ! {
    rust_deinit(code, core::ptr::null());
    unsafe {
        libc::exit(code);
    }
}

#[no_mangle]
pub extern "C" fn rust_init(argc: i32, argv: *const *const i8) {
    arch::env::init(argc, argv);
    com::pre_init();
    syscalls::init();
    tiles::init();
    com::init();
    io::init();
    arch::tcu::init();

    if let Some(func) = arch::env::get().load_func() {
        let res = func();
        exit(res);
    }
}

#[no_mangle]
pub extern "C" fn rust_deinit(status: i32, _arg: *const libc::c_void) {
    io::deinit();
    vfs::deinit();
    syscalls::activity_ctrl(
        tiles::Activity::own().sel(),
        kif::syscalls::ActivityOp::STOP,
        status as u64,
    )
    .unwrap();
    arch::tcu::deinit();
}
