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

#![feature(llvm_asm)]
#![feature(const_fn)]
#![feature(core_intrinsics)]
#![no_std]

#[macro_use]
extern crate base;
#[macro_use]
extern crate cfg_if;
extern crate paging;

mod arch;
mod corereq;
mod helper;
mod pexcalls;
mod timer;
mod upcalls;
mod vma;
mod vpe;

use base::cell::StaticCell;
use base::cfg;
use base::envdata;
use base::goff;
use base::io;
use base::kif;
use base::libc;
use base::tcu;
use base::util;
use core::intrinsics;

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
/// Logs page table allocations/frees
pub const LOG_PTS: bool = false;
/// Logs timer IRQs
pub const LOG_TIMER: bool = false;

extern "C" {
    fn heap_init(begin: usize, end: usize);
    fn gem5_shutdown(delay: u64);
}

// the heap area needs to be 16-byte aligned
#[repr(align(16))]
struct Heap([u64; 8 * 1024]);
#[used]
static mut HEAP: Heap = Heap { 0: [0; 8 * 1024] };

pub struct PEXEnv {
    pe_id: u64,
    pe_desc: kif::PEDesc,
    mem_start: goff,
    mem_end: goff,
}

static PEX_ENV: StaticCell<PEXEnv> = StaticCell::new(PEXEnv {
    pe_id: 0,
    pe_desc: kif::PEDesc::new_from(0),
    mem_start: 0,
    mem_end: 0,
});

pub fn pex_env() -> &'static PEXEnv {
    PEX_ENV.get()
}

pub fn app_env() -> &'static mut envdata::EnvData {
    unsafe { intrinsics::transmute(cfg::ENV_START) }
}

pub struct PagefaultMessage {
    pub op: u64,
    pub virt: u64,
    pub access: u64,
}

// ensure that there is no page-boundary within the messages
#[repr(align(4096))]
pub struct Messages {
    pub pagefault: PagefaultMessage,
    pub exit_notify: kif::pemux::Exit,
    pub upcall_reply: kif::pemux::Response,
}

static MSGS: StaticCell<Messages> = StaticCell::new(Messages {
    pagefault: PagefaultMessage {
        op: 0,
        virt: 0,
        access: 0,
    },
    exit_notify: kif::pemux::Exit {
        code: 0,
        op: 0,
        vpe_sel: 0,
    },
    upcall_reply: kif::pemux::Response { error: 0, val: 0 },
});

pub fn msgs_mut() -> &'static mut Messages {
    const_assert!(util::size_of::<Messages>() <= cfg::PAGE_SIZE);
    MSGS.get_mut()
}

#[no_mangle]
pub extern "C" fn abort() {
    exit(1);
}

#[no_mangle]
pub extern "C" fn exit(_code: i32) {
    unsafe { gem5_shutdown(0) };
}

static SCHED: StaticCell<Option<vpe::ScheduleAction>> = StaticCell::new(None);

#[inline]
fn leave(state: &mut arch::State) -> *mut libc::c_void {
    upcalls::check();

    if let Some(action) = SCHED.set(None) {
        vpe::schedule(action) as *mut libc::c_void
    }
    else {
        state as *mut _ as *mut libc::c_void
    }
}

pub fn reg_scheduling(action: vpe::ScheduleAction) {
    SCHED.set(Some(action));
}

pub fn scheduling_pending() -> bool {
    SCHED.is_some()
}

pub extern "C" fn unexpected_irq(state: &mut arch::State) -> *mut libc::c_void {
    log!(LOG_ERR, "Unexpected IRQ with {:?}", state);
    vpe::remove_cur(1);

    leave(state)
}

#[cfg(any(target_arch = "riscv64", target_arch = "x86_64"))]
pub extern "C" fn fpu_ex(state: &mut arch::State) -> *mut libc::c_void {
    arch::handle_fpu_ex(state);
    leave(state)
}

pub extern "C" fn mmu_pf(state: &mut arch::State) -> *mut libc::c_void {
    if arch::handle_mmu_pf(state).is_err() {
        vpe::remove_cur(1);
    }

    leave(state)
}

pub extern "C" fn pexcall(state: &mut arch::State) -> *mut libc::c_void {
    pexcalls::handle_call(state);

    leave(state)
}

pub extern "C" fn tcu_irq(state: &mut arch::State) -> *mut libc::c_void {
    // on ARM, we use the same IRQ for both core requests and the timer
    cfg_if! {
        if #[cfg(target_arch = "arm")] {
            if tcu::TCU::get_irq() == tcu::IRQ::TIMER {
                return timer_irq(state);
            }
        }
    }

    tcu::TCU::clear_irq(tcu::IRQ::CORE_REQ);

    // core request from TCU?
    let core_req = tcu::TCU::get_core_req();
    if core_req != 0 {
        if (core_req & 0x1) != 0 {
            corereq::handle_recv(core_req);
        }
        else {
            vma::handle_xlate(core_req)
        }
    }

    leave(state)
}

pub extern "C" fn timer_irq(state: &mut arch::State) -> *mut libc::c_void {
    tcu::TCU::clear_irq(tcu::IRQ::TIMER);

    vpe::cur().consume_time();
    timer::trigger();

    leave(state)
}

#[no_mangle]
pub extern "C" fn init() -> usize {
    // switch to a different VPE during the init phase to ensure that we don't miss messages for us
    let old_id = tcu::TCU::xchg_vpe(0);
    assert!((old_id >> 16) == 0);

    // init our own environment; at this point we can still access app_env, because it is mapped by
    // the gem5 loader for us. afterwards, our address space does not contain that anymore.s
    PEX_ENV.get_mut().pe_id = app_env().pe_id;
    PEX_ENV.get_mut().pe_desc = kif::PEDesc::new_from(app_env().pe_desc);
    PEX_ENV.get_mut().mem_start = app_env().pe_mem_base;
    PEX_ENV.get_mut().mem_end = app_env().pe_mem_base + cfg::PEMUX_START as u64;
    assert!(app_env().pe_mem_size >= cfg::PEMUX_START as u64);

    unsafe {
        heap_init(
            &HEAP.0 as *const u64 as usize,
            &HEAP.0 as *const u64 as usize + HEAP.0.len() * 8,
        );
    }

    io::init(pex_env().pe_id, "pemux");
    vpe::init();

    // switch to idle
    vpe::idle().start();
    vpe::schedule(vpe::ScheduleAction::Yield);

    let stack_top = vpe::idle().user_state() as *const _ as usize + util::size_of::<arch::State>();
    arch::init(stack_top);
    stack_top
}
