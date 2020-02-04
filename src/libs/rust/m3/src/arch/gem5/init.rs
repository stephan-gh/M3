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

use arch;
use base::pexif;
use com;
use io;
use mem;
use pes;
use syscalls;
use vfs;

#[no_mangle]
pub extern "C" fn abort() -> ! {
    exit(1);
}

#[no_mangle]
pub extern "C" fn exit(code: i32) -> ! {
    io::deinit();
    vfs::deinit();
    arch::pexcalls::call1(pexif::Operation::EXIT, code as usize).ok();
    unreachable!();
}

extern "C" {
    fn main() -> i32;
}

#[no_mangle]
pub extern "C" fn env_run() {
    let res = if arch::env::get().has_lambda() {
        syscalls::reinit();
        io::reinit();
        pes::reinit();
        arch::env::closure().call()
    }
    else {
        mem::heap::init();
        pes::init();
        syscalls::init();
        io::init();
        com::init();
        unsafe { main() }
    };
    exit(res)
}
