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
#![feature(const_fn)]
#![feature(core_intrinsics)]
#![no_std]

#[macro_use]
extern crate base;
extern crate paging;

mod arch;
mod corereq;
mod helper;
mod pexcalls;
mod upcalls;
mod vma;
mod vpe;

use base::cell::StaticCell;
use base::cfg;
use base::dtu;
use base::envdata;
use base::io;
use base::kif;
use base::libc;
use core::intrinsics;
use core::ptr;

/// Logs pexcalls
pub const LOG_CALLS: bool = false;
/// Logs VPE operations
pub const LOG_VPES: bool = false;
/// Logs upcalls
pub const LOG_UPCALLS: bool = false;
/// Logs foreign messages
pub const LOG_FOREIGN_MSG: bool = false;

extern "C" {
    fn heap_init(begin: usize, end: usize);
    fn gem5_shutdown(delay: u64);
}

#[used]
static mut HEAP: [u64; 8 * 1024] = [0; 8 * 1024];

#[no_mangle]
pub extern "C" fn abort() {
    exit(1);
}

#[no_mangle]
pub extern "C" fn exit(_code: i32) {
    unsafe { gem5_shutdown(0) };
}

pub fn env() -> &'static mut envdata::EnvData {
    unsafe { intrinsics::transmute(cfg::ENV_START) }
}

#[no_mangle]
pub fn sleep() {
    loop {
        // ack events since to VPE is currently not running
        dtu::DTU::fetch_events();
        dtu::DTU::sleep().ok();
    }
}

static STOPPED: StaticCell<bool> = StaticCell::new(false);
static NESTING_LEVEL: StaticCell<u32> = StaticCell::new(0);

fn enter() {
    *NESTING_LEVEL.get_mut() += 1;
}

fn leave(state: &mut arch::State) -> *mut libc::c_void {
    if *STOPPED {
        stop_vpe(state);
    }
    *NESTING_LEVEL.get_mut() -= 1;
    state as *mut _ as *mut libc::c_void
}

pub fn stop_vpe(state: &mut arch::State) {
    if *NESTING_LEVEL > 1 {
        // prevent us from sleeping by setting the user event
        dtu::DTU::set_event().ok();

        *STOPPED.get_mut() = true;
    }
    else {
        state.stop();

        *STOPPED.get_mut() = false;
    }
}

pub fn is_stopped() -> bool {
    // use volatile because STOPPED may have changed via a nested IRQ
    unsafe { ptr::read_volatile(STOPPED.get_mut()) }
}

pub extern "C" fn unexpected_irq(state: &mut arch::State) -> *mut libc::c_void {
    panic!("Unexpected IRQ with {:?}", state);
}

pub extern "C" fn mmu_pf(state: &mut arch::State) -> *mut libc::c_void {
    enter();

    arch::handle_mmu_pf(state);

    upcalls::check(state);

    leave(state)
}

pub extern "C" fn pexcall(state: &mut arch::State) -> *mut libc::c_void {
    enter();

    pexcalls::handle_call(state);

    upcalls::check(state);

    leave(state)
}

pub extern "C" fn dtu_irq(state: &mut arch::State) -> *mut libc::c_void {
    enter();

    #[cfg(target_arch = "arm")]
    dtu::DTU::clear_irq();

    // core request from DTU?
    let core_req = dtu::DTU::get_core_req();
    if core_req != 0 {
        // acknowledge the request
        dtu::DTU::set_core_req(0);

        if (core_req & 0x1) != 0 {
            corereq::handle_recv(core_req);
        }
        else {
            vma::handle_xlate(core_req)
        }
    }

    upcalls::check(state);

    leave(state)
}

#[no_mangle]
pub extern "C" fn init() {
    unsafe {
        arch::init();

        heap_init(
            &HEAP as *const u64 as usize,
            &HEAP as *const u64 as usize + HEAP.len() * 8,
        );
    }

    io::init(0, "pemux");
    vpe::init(kif::PEDesc::new_from(env().pe_desc), env().pe_mem_base, env().pe_mem_size);
    dtu::DTU::xchg_vpe(vpe::cur().vpe_reg());
}
