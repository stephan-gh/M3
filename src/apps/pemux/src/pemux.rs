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
use base::tcu;
use base::envdata;
use base::io;
use base::kif;
use base::libc;
use core::intrinsics;
use core::ptr;

/// Logs errors
pub const LOG_ERR: bool = true;
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
        tcu::TCU::fetch_events();
        tcu::TCU::sleep().ok();
    }
}

static STOPPED: StaticCell<bool> = StaticCell::new(false);
static NESTING_LEVEL: StaticCell<u32> = StaticCell::new(0);

fn enter() {
    *NESTING_LEVEL.get_mut() += 1;
}

fn leave(state: &mut arch::State) -> *mut libc::c_void {
    upcalls::check(state);

    if *STOPPED {
        stop_vpe(state);
    }
    *NESTING_LEVEL.get_mut() -= 1;
    state as *mut _ as *mut libc::c_void
}

fn set_user_event() {
    // we can assume here that we (PEMux) is the current VPE, because we call it only from stop_vpe
    // with NESTING_LEVEL > 1.
    let our = vpe::our();
    let old_vpe = tcu::TCU::xchg_vpe(vpe::idle().vpe_reg());
    // set user event
    our.set_vpe_reg(old_vpe | tcu::EventMask::USER.bits());
    // switch back; we don't need to restore the VPE reg of idle; it doesn't receive msgs anyway
    tcu::TCU::xchg_vpe(our.vpe_reg());
}

pub fn nesting_level() -> u32 {
    *NESTING_LEVEL
}

pub fn stop_vpe(state: &mut arch::State) {
    if *NESTING_LEVEL > 1 {
        // prevent us from sleeping by setting the user event
        set_user_event();

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
    enter();

    log!(LOG_ERR, "Unexpected IRQ with {:?}", state);
    stop_vpe(state);
    vpe::remove(1, true);

    leave(state)
}

pub extern "C" fn mmu_pf(state: &mut arch::State) -> *mut libc::c_void {
    enter();

    if arch::handle_mmu_pf(state).is_err() {
        stop_vpe(state);
        vpe::remove(1, true);
    }

    leave(state)
}

pub extern "C" fn pexcall(state: &mut arch::State) -> *mut libc::c_void {
    enter();

    pexcalls::handle_call(state);

    leave(state)
}

pub extern "C" fn tcu_irq(state: &mut arch::State) -> *mut libc::c_void {
    enter();

    #[cfg(any(target_arch = "arm", target_arch = "riscv64"))]
    tcu::TCU::clear_irq();

    // core request from TCU?
    let core_req = tcu::TCU::get_core_req();
    if core_req != 0 {
        // acknowledge the request
        tcu::TCU::set_core_req(0);

        if (core_req & 0x1) != 0 {
            corereq::handle_recv(core_req);
        }
        else {
            vma::handle_xlate(core_req)
        }
    }

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
    tcu::TCU::xchg_vpe(vpe::cur().vpe_reg());
}
