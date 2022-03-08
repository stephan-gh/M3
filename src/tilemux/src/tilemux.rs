/*
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

#![feature(asm)]
#![feature(const_fn)]
#![feature(core_intrinsics)]
#![feature(duration_constants)]
#![no_std]

#[allow(unused_extern_crates)]
extern crate heap;

mod activities;
mod arch;
mod corereq;
mod helper;
mod irqs;
mod quota;
mod sendqueue;
mod sidecalls;
mod timer;
mod tmcalls;
mod vma;

use base::cell::{Ref, StaticCell, StaticRefCell};
use base::cfg;
use base::envdata;
use base::io;
use base::kif;
use base::libc;
use base::log;
use base::machine;
use base::mem;
use base::tcu;

/// Logs errors
pub const LOG_ERR: bool = true;
/// Logs basic activity operations
pub const LOG_ACTS: bool = false;
/// Logs tmcalls
pub const LOG_CALLS: bool = false;
/// Logs context switches
pub const LOG_CTXSWS: bool = false;
/// Logs sidecalls
pub const LOG_SIDECALLS: bool = false;
/// Logs foreign messages
pub const LOG_FOREIGN_MSG: bool = false;
/// Logs core requests
pub const LOG_CORE_REQS: bool = false;
/// Logs page table allocations/frees
pub const LOG_PTS: bool = false;
/// Logs timer IRQs
pub const LOG_TIMER: bool = false;
/// Logs interrupts
pub const LOG_IRQS: bool = false;
/// Logs sendqueue operations
pub const LOG_SQUEUE: bool = false;

extern "C" {
    fn heap_init(begin: usize, end: usize);
}

// the heap area needs to be 16-byte aligned
#[repr(align(16))]
struct Heap([u64; 8 * 1024]);
#[used]
static mut HEAP: Heap = Heap { 0: [0; 8 * 1024] };

pub struct TMEnv {
    tile_id: u64,
    tile_desc: kif::TileDesc,
    platform: u64,
}

static TM_ENV: StaticRefCell<TMEnv> = StaticRefCell::new(TMEnv {
    tile_id: 0,
    tile_desc: kif::TileDesc::new_from(0),
    platform: 0,
});

pub fn pex_env() -> Ref<'static, TMEnv> {
    TM_ENV.borrow()
}

pub fn app_env() -> &'static mut envdata::EnvData {
    unsafe { &mut *(cfg::ENV_START as *mut _) }
}

pub struct PagefaultMessage {
    pub op: u64,
    pub virt: u64,
    pub access: u64,
}

#[no_mangle]
pub extern "C" fn abort() {
    exit(1);
}

#[no_mangle]
pub extern "C" fn exit(_code: i32) {
    machine::shutdown();
}

static SCHED: StaticCell<Option<activities::ScheduleAction>> = StaticCell::new(None);

#[inline]
fn leave(state: &mut arch::State) -> *mut libc::c_void {
    sidecalls::check();

    if let Some(action) = SCHED.replace(None) {
        activities::schedule(action) as *mut libc::c_void
    }
    else {
        state as *mut _ as *mut libc::c_void
    }
}

pub fn reg_scheduling(action: activities::ScheduleAction) {
    SCHED.set(Some(action));
}

pub fn scheduling_pending() -> bool {
    SCHED.get().is_some()
}

pub extern "C" fn unexpected_irq(state: &mut arch::State) -> *mut libc::c_void {
    log!(LOG_ERR, "Unexpected IRQ with user state:\n{:?}", state);
    activities::remove_cur(1);

    leave(state)
}

#[cfg(any(target_arch = "riscv64", target_arch = "x86_64"))]
pub extern "C" fn fpu_ex(state: &mut arch::State) -> *mut libc::c_void {
    arch::handle_fpu_ex(state);
    leave(state)
}

pub extern "C" fn mmu_pf(state: &mut arch::State) -> *mut libc::c_void {
    if arch::handle_mmu_pf(state).is_err() {
        activities::remove_cur(1);
    }

    leave(state)
}

pub extern "C" fn tmcall(state: &mut arch::State) -> *mut libc::c_void {
    tmcalls::handle_call(state);

    leave(state)
}

pub extern "C" fn ext_irq(state: &mut arch::State) -> *mut libc::c_void {
    match isr::get_irq() {
        isr::IRQSource::TCU(tcu::IRQ::TIMER) => {
            activities::cur().consume_time();
            timer::trigger();
        },

        isr::IRQSource::TCU(tcu::IRQ::CORE_REQ) => {
            if let Some(r) = tcu::TCU::get_core_req() {
                log!(crate::LOG_CORE_REQS, "Got {:x?}", r);
                corereq::handle_recv(r);
            }
        },

        isr::IRQSource::Ext(id) => {
            irqs::signal(id);
        },

        n => log!(crate::LOG_ERR, "Unexpected IRQ {:?}", n),
    }

    leave(state)
}

#[no_mangle]
pub extern "C" fn init() -> usize {
    // switch to a different activity during the init phase to ensure that we don't miss messages for us
    let old_id = tcu::TCU::xchg_activity(0).unwrap();
    assert!((old_id >> 16) == 0);

    // init our own environment; at this point we can still access app_env, because it is mapped by
    // the gem5 loader for us. afterwards, our address space does not contain that anymore.s
    {
        let mut env = TM_ENV.borrow_mut();
        env.tile_id = app_env().tile_id;
        env.tile_desc = kif::TileDesc::new_from(app_env().tile_desc);
        env.platform = app_env().platform;
    }

    unsafe {
        heap_init(
            &HEAP.0 as *const u64 as usize,
            &HEAP.0 as *const u64 as usize + HEAP.0.len() * 8,
        );
    }

    io::init(pex_env().tile_id, "tilemux");
    activities::init();

    // switch to idle
    activities::idle().start();
    activities::schedule(activities::ScheduleAction::Yield);

    let state = activities::idle().user_state();
    let state_top = state as *const _ as usize + mem::size_of::<arch::State>();
    arch::init(state);
    // store platform already in app env, because we need it for logging
    app_env().platform = pex_env().platform;

    state_top
}
