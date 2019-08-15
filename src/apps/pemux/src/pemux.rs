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

#![feature(asm)]
#![feature(core_intrinsics)]
#![no_std]

#[macro_use]
extern crate base;

mod arch;
mod kernreq;
mod pexcalls;

use base::dtu;
use base::io;
use base::libc;

use arch::isr;
use arch::vma;

extern "C" {
    fn heap_init(begin: usize, end: usize);
    fn gem5_shutdown(delay: u64);
}

#[used]
static mut HEAP: [u64; 8 * 1024] = [0; 8 * 1024];

#[no_mangle]
pub extern "C" fn exit(_code: i32) {
    unsafe { gem5_shutdown(0) };
}

#[no_mangle]
pub fn sleep() {
    loop {
        dtu::DTU::sleep().ok();
    }
}

pub extern "C" fn unexpected_irq(state: &mut isr::State) -> *mut libc::c_void {
    panic!("Unexpected IRQ with {:?}", state);
}

pub extern "C" fn mmu_pf(state: &mut isr::State) -> *mut libc::c_void {
    vma::handle_mmu_pf(state);

    state as *mut isr::State as *mut libc::c_void
}

pub extern "C" fn pexcall(state: &mut isr::State) -> *mut libc::c_void {
    pexcalls::handle_call(state);

    state as *mut isr::State as *mut libc::c_void
}

pub extern "C" fn dtu_irq(state: &mut isr::State) -> *mut libc::c_void {
    // translation request from DTU?
    let xlate_req = dtu::DTU::get_xlate_req();
    if xlate_req != 0 {
        vma::handle_xlate(state, xlate_req)
    }

    // request from the kernel?
    let ext_req = dtu::DTU::get_ext_req();
    if ext_req != 0 {
        kernreq::handle_ext_req(state, ext_req);
    }

    state as *mut isr::State as *mut libc::c_void
}

#[no_mangle]
pub extern "C" fn init() {
    unsafe {
        isr::init();

        // init heap
        heap_init(
            &HEAP as *const u64 as usize,
            &HEAP as *const u64 as usize + HEAP.len() * 8,
        );

        io::init(0, "pemux");
    }
}
