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
use base::envdata;
use base::io;
use base::kif;
use base::libc;
use base::tcu;
use base::util;
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

    static isr_stack_low: libc::c_void;
}

#[used]
static mut HEAP: [u64; 8 * 1024] = [0; 8 * 1024];

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
    pub upcall_reply: kif::DefaultReply,
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
    upcall_reply: kif::DefaultReply { error: 0 },
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

static SCHED: StaticCell<bool> = StaticCell::new(false);
static STOPPED: StaticCell<bool> = StaticCell::new(false);
static NESTING_LEVEL: StaticCell<u32> = StaticCell::new(0);

fn enter() {
    *NESTING_LEVEL.get_mut() += 1;
}

fn leave(state: &mut arch::State) -> *mut libc::c_void {
    upcalls::check();

    if *STOPPED && *NESTING_LEVEL == 1 {
        // status and notify don't matter, because we've already called vpe::remove in a deeper
        // nesting level and thus, the VPE is gone.
        stop_vpe(0, false);
    }
    *NESTING_LEVEL.get_mut() -= 1;

    if SCHED.set(false) {
        vpe::schedule(state as *mut _ as usize, false) as *mut libc::c_void
    }
    else {
        state as *mut _ as *mut libc::c_void
    }
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

pub fn reg_scheduling() {
    SCHED.set(true);
}

pub fn nesting_level() -> u32 {
    *NESTING_LEVEL
}

pub fn stop_vpe(status: u32, notify: bool) {
    vpe::remove(status, notify);

    if *NESTING_LEVEL > 1 {
        // prevent us from sleeping by setting the user event
        set_user_event();

        *STOPPED.get_mut() = true;
    }
    else {
        // TODO remove user event

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
    stop_vpe(1, true);

    leave(state)
}

pub extern "C" fn mmu_pf(state: &mut arch::State) -> *mut libc::c_void {
    enter();

    if arch::handle_mmu_pf(state).is_err() {
        stop_vpe(1, true);
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

    io::init(env().pe_id, "pemux");
    vpe::init(
        env().pe_id,
        kif::PEDesc::new_from(env().pe_desc),
        env().pe_mem_base,
        env().pe_mem_size,
    );

    // disable upcalls, because we can't handle them if our own VPE is the current one
    let _upcalls_off = helper::UpcallsOffGuard::new();
    // the kernel sends us an initial upcall, to which we reply to force an TLB miss in the TCU and
    // put our "messages page" into the TLB as a fixed entry. afterwards, we can leave IRQs off when
    // sending messages, because we know that the page is present in the TLB.
    loop {
        tcu::TCU::fetch_events();
        tcu::TCU::sleep().ok();

        if let Some(msg) = tcu::TCU::fetch_msg(tcu::PEXUP_REP) {
            // enable interrupts for address translations
            let _guard = helper::IRQsOnGuard::new();

            let reply = &mut msgs_mut().upcall_reply;
            reply.error = 0;
            tcu::TCU::reply(
                tcu::PEXUP_REP,
                reply as *const _ as *const u8,
                util::size_of::<kif::DefaultReply>(),
                msg,
            ).unwrap();
            break;
        }
    }

    // switch to idle
    let state_addr = unsafe { &isr_stack_low as *const _ as usize };
    vpe::schedule(state_addr - util::size_of::<arch::State>(), false);
}
