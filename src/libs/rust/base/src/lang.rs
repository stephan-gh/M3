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

use core::intrinsics;
use core::panic::PanicInfo;

use crate::backtrace;
use crate::io::{log, Write};

extern "C" {
    fn exit(code: i32);
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // disable instruction trace here to see the instructions that lead to the panic
    #[cfg(target_vendor = "hw")]
    crate::tcu::TCU::set_trace_instrs(false);

    if let Some(l) = log::Log::get() {
        if let Some(loc) = info.location() {
            l.write_fmt(format_args!(
                "PANIC at {}, line {}, column {}: ",
                loc.file(),
                loc.line(),
                loc.column()
            ))
            .unwrap();
        }
        else {
            l.write(b"PANIC at unknown location: ").unwrap();
        }
        if let Some(msg) = info.message() {
            l.write_fmt(*msg).unwrap();
        }
        l.write(b"\n\n").unwrap();

        let mut bt = [0usize; 16];
        let bt_len = backtrace::collect(&mut bt);
        l.write(b"Backtrace:\n").unwrap();
        for addr in bt.iter().take(bt_len) {
            l.write_fmt(format_args!("  {:#x}\n", addr)).unwrap();
        }
    }

    unsafe {
        exit(1)
    };
    intrinsics::abort();
}

#[alloc_error_handler]
fn alloc_error(_: core::alloc::Layout) -> ! {
    panic!("Alloc error");
}

#[lang = "eh_personality"]
#[no_mangle]
#[doc(hidden)]
pub extern "C" fn rust_eh_personality() {
    intrinsics::abort()
}

#[allow(non_snake_case)]
#[no_mangle]
#[doc(hidden)]
pub extern "C" fn _Unwind_Resume() -> ! {
    intrinsics::abort()
}

#[cfg(target_arch = "arm")]
#[no_mangle]
#[doc(hidden)]
pub unsafe extern "C" fn __aeabi_memclr(dest: *mut crate::libc::c_void, size: usize) {
    crate::libc::memzero(dest, size);
}

#[cfg(target_arch = "arm")]
#[no_mangle]
#[doc(hidden)]
pub unsafe extern "C" fn __aeabi_memclr4(dest: *mut crate::libc::c_void, size: usize) {
    crate::libc::memzero(dest, size);
}
