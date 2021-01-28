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

use crate::arch;
use crate::com;
use crate::io;
use crate::mem;
use crate::pes;
use crate::syscalls;
use crate::vfs;

#[no_mangle]
pub extern "C" fn abort() -> ! {
    exit(1);
}

#[no_mangle]
pub extern "C" fn exit(_code: i32) -> ! {
    io::deinit();
    vfs::deinit();

    #[cfg(target_vendor = "gem5")]
    crate::pexcalls::exit(_code);

    #[cfg(target_vendor = "hw")]
    {
        unsafe {
            llvm_asm!(
                "jr $0"
                // set x10 to tell crt0 that the SP is not set
                : : "r"(crate::cfg::PEMUX_START), "{x10}"(0u64)
            );
        }
        unreachable!();
    }
}

extern "C" {
    fn main() -> i32;
}

#[no_mangle]
pub extern "C" fn env_run() {
    let res = if arch::env::get().has_lambda() {
        syscalls::reinit();
        com::pre_init();
        io::reinit();
        pes::reinit();
        arch::env::closure().call()
    }
    else {
        mem::heap::init();
        syscalls::init();
        com::pre_init();
        pes::init();
        io::init();
        com::init();
        unsafe { main() }
    };
    exit(res)
}
