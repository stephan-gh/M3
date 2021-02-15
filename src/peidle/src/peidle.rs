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
#![feature(core_intrinsics)]
#![no_std]

extern crate base;
extern crate heap;

use base::cell::{LazyStaticCell, StaticCell};
use base::cfg;
use base::envdata;
use base::errors::{Code, Error};
use base::io;
use base::kif;
use base::libc;
use base::log;
use base::machine;
use base::mem;
use base::pexif;
use base::tcu;

const UPC_RBUF_ADDR: usize = cfg::PEMUX_RBUF_SPACE + cfg::KPEX_RBUF_SIZE;

/// Logs upcalls
pub const LOG_INFO: bool = true;
pub const LOG_VERBOSE: bool = false;
pub const LOG_PEXCALLS: bool = false;

// remember our PE id here, because the environment is overwritten later
static PE_ID: StaticCell<u64> = StaticCell::new(0);
static CUR_VPE: StaticCell<Option<u64>> = StaticCell::new(None);

static STATE: LazyStaticCell<isr::State> = LazyStaticCell::default();

#[no_mangle]
pub extern "C" fn abort() -> ! {
    exit(1);
}

#[no_mangle]
pub extern "C" fn exit(_code: i32) -> ! {
    machine::shutdown();
}

pub fn app_env() -> &'static mut envdata::EnvData {
    unsafe { &mut *(cfg::ENV_START as *mut _) }
}

fn reply_msg<T>(msg: &'static tcu::Message, reply: &T) {
    let msg_off = tcu::TCU::msg_to_offset(UPC_RBUF_ADDR, msg);
    tcu::TCU::reply(
        tcu::PEXUP_REP,
        reply as *const T as *const u8,
        mem::size_of::<T>(),
        msg_off,
    )
    .unwrap();
}

fn vpe_ctrl(msg: &'static tcu::Message) -> Result<Option<(usize, usize)>, Error> {
    let req = msg.get_data::<kif::pemux::VPECtrl>();

    let vpe_id = req.vpe_sel;
    let op = kif::pemux::VPEOp::from(req.vpe_op);
    let eps_start = req.eps_start as tcu::EpId;

    log!(
        crate::LOG_INFO,
        "upcall::vpe_ctrl(vpe={}, op={:?}, eps_start={})",
        vpe_id,
        op,
        eps_start
    );

    match op {
        kif::pemux::VPEOp::INIT => {
            assert!(CUR_VPE.is_none());
            CUR_VPE.set(Some(vpe_id));
            Ok(None)
        },

        kif::pemux::VPEOp::START => {
            assert!(CUR_VPE.is_some());
            // we can run the app now
            Ok(Some((app_env().entry as usize, app_env().sp as usize)))
        },

        // for all others, "stop" the VPE
        _ => {
            CUR_VPE.set(None);
            Ok(None)
        },
    }
}

fn handle_upcall(msg: &'static tcu::Message) -> Option<(usize, usize)> {
    let req = msg.get_data::<kif::DefaultRequest>();
    let opcode = kif::pemux::Upcalls::from(req.opcode);

    log!(
        crate::LOG_VERBOSE,
        "received upcall {:?}: {:?}",
        opcode,
        msg
    );

    let res = match opcode {
        kif::pemux::Upcalls::VPE_CTRL => vpe_ctrl(msg),
        _ => Err(Error::new(Code::NotSup)),
    };

    let mut reply = kif::pemux::Response { error: 0, val: 0 };
    reply.val = 0;
    reply.error = match res {
        Ok(_) => 0,
        Err(ref e) => e.code() as u64,
    };
    reply_msg(msg, &reply);
    res.unwrap_or(None)
}

const PEXC_ARG0: usize = 9; // a0 = x10
const PEXC_ARG1: usize = 10; // a1 = x11

pub extern "C" fn handle_pexcall(state: &mut isr::State) -> *mut libc::c_void {
    let call = pexif::Operation::from(state.r[PEXC_ARG0] as isize);

    log!(
        crate::LOG_PEXCALLS,
        "PEXCall: {}(arg1={})",
        call,
        state.r[PEXC_ARG1]
    );

    let res = match call {
        pexif::Operation::EXIT => exit_app(state),
        pexif::Operation::FLUSH_INV => flush_invalidate(),
        pexif::Operation::SLEEP => Ok(0),
        pexif::Operation::YIELD => Ok(0),
        pexif::Operation::NOOP => Ok(0),

        _ => Err(Error::new(Code::NotSup)),
    };

    state.r[PEXC_ARG0] = res.unwrap_or_else(|e| -(e.code() as isize)) as usize;

    state as *mut _ as *mut libc::c_void
}

fn flush_invalidate() -> Result<isize, Error> {
    unsafe {
        llvm_asm!("fence.i" : : : : "volatile");
    }
    Ok(0)
}

fn run_app(entry: usize, sp: usize) -> ! {
    log!(
        crate::LOG_INFO,
        "Running app with entry={:#x} sp={:#x}",
        entry,
        sp
    );

    // enable instruction trace again for the app we're about to run
    tcu::TCU::set_trace_instrs(true);

    // let app know the PE its running on and that it's not shared with others
    app_env().pe_id = *PE_ID;
    app_env().shared = 0;

    unsafe {
        llvm_asm!(
            concat!(
                // enable FPU
                "li     t0, 1 << 13\n",
                "csrs   sstatus, t0\n",
                // return to user mode
                "li     t0, 1 << 8\n",
                "csrc   sstatus, t0\n",
                // enable interrupts
                "li     t0, 1 << 5\n",
                "csrs   sstatus, t0\n",
                // return to entry point
                "csrw   sepc, $0\n",
                ".global __app_start\n",
                "sret\n",
            )
            // set SP and set x10 to tell crt0 that the SP is already set
            : : "r"(entry), "{x2}"(sp), "{x10}"(0xDEAD_BEEFu64)
            : "t0"
            : : "volatile"
        );
    }
    unreachable!();
}

fn exit_app(state: &mut isr::State) -> Result<isize, Error> {
    // disable instruction trace to see the last instructions of the previous app
    tcu::TCU::set_trace_instrs(false);

    let vpe = CUR_VPE.get_mut().take().unwrap();
    let code = state.r[PEXC_ARG1] as i32;

    let msg = kif::pemux::Exit {
        op: kif::pemux::Calls::EXIT.val as u64,
        vpe_sel: vpe,
        code: code as u64,
    };

    log!(crate::LOG_INFO, "Sending exit for VPE {}", vpe);

    let msg_addr = &msg as *const _ as *const u8;
    let size = mem::size_of::<kif::pemux::Exit>();
    tcu::TCU::send(tcu::KPEX_SEP, msg_addr, size, 0, tcu::KPEX_REP).ok();

    // sync icache with dcache
    flush_invalidate().ok();

    // wait for next app
    wait_for_app();
    unreachable!();
}

fn wait_for_app() {
    // wait until the app can be started
    let (entry, sp) = loop {
        if envdata::get().platform == envdata::Platform::GEM5.val {
            tcu::TCU::sleep().ok();
        }

        if let Some(msg_off) = tcu::TCU::fetch_msg(tcu::PEXUP_REP) {
            let msg = tcu::TCU::offset_to_msg(UPC_RBUF_ADDR, msg_off);
            if let Some(res) = handle_upcall(msg) {
                break res;
            }
        }

        // just ACK replies from the kernel; we don't care about them
        if let Some(msg_off) = tcu::TCU::fetch_msg(tcu::KPEX_REP) {
            tcu::TCU::ack_msg(tcu::KPEX_REP, msg_off).unwrap();
        }
    };

    run_app(entry, sp);
}

#[no_mangle]
pub extern "C" fn env_run() {
    PE_ID.set(envdata::get().pe_id);

    // install exception handlers to ease debugging
    STATE.set(isr::State::default());
    isr::init(STATE.get_mut());
    isr::reg(isr::Vector::ENV_UCALL.val, handle_pexcall);
    isr::reg(isr::Vector::ENV_SCALL.val, handle_pexcall);
    isr::enable_irqs();

    io::init(*PE_ID, "pemux");

    // wait until the kernel configured the EP (only necessary on HW where we can't sleep below)
    if envdata::get().platform == envdata::Platform::HW.val {
        loop {
            if tcu::TCU::is_valid(tcu::PEXUP_REP)
                && tcu::TCU::is_valid(tcu::KPEX_SEP)
                && tcu::TCU::is_valid(tcu::KPEX_REP)
            {
                break;
            }
        }
    }

    wait_for_app();
}
