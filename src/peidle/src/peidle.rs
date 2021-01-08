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
use base::log;
use base::machine;
use base::tcu;
use base::util;

const UPC_RBUF_ADDR: usize = cfg::PEMUX_RBUF_SPACE + cfg::KPEX_RBUF_SIZE;

/// Logs upcalls
pub const LOG_INFO: bool = true;
pub const LOG_VERBOSE: bool = false;

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
        util::size_of::<T>(),
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

        // ignore all other requests
        _ => Ok(None),
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

fn run_app(entry: usize, sp: usize) -> ! {
    log!(
        crate::LOG_INFO,
        "Running app with entry={:#x} sp={:#x}",
        entry,
        sp
    );

    // let app know the PE its running on and that it's not shared with others
    app_env().pe_id = *PE_ID;
    app_env().shared = 0;

    unsafe {
        llvm_asm!(
            concat!(
                // enable FPU
                "li t0, 1 << 13\n",
                "csrs sstatus, t0\n",
                // jump to entry point
                ".global __app_start\n",
                "__app_start: jr $0\n"
            )
            // set SP and set x10 to tell crt0 that the SP is already set
            : : "r"(entry), "{x2}"(sp), "{x10}"(0xDEAD_BEEFu64)
            : "t0"
            : : "volatile"
        );
    }
    unreachable!();
}

fn send_exit(vpe: u64) {
    let msg = kif::pemux::Exit {
        op: kif::pemux::Calls::EXIT.val as u64,
        vpe_sel: vpe,
        code: 0,
    };

    log!(crate::LOG_INFO, "Sending exit for VPE {}", vpe);

    let msg_addr = &msg as *const _ as *const u8;
    let size = util::size_of::<kif::pemux::Exit>();
    tcu::TCU::send(tcu::KPEX_SEP, msg_addr, size, 0, tcu::KPEX_REP).ok();
}

#[no_mangle]
pub extern "C" fn env_run() {
    if *PE_ID == 0 {
        PE_ID.set(envdata::get().pe_id);

        // install exception handlers to ease debugging
        STATE.set(isr::State::default());
        isr::init(STATE.get_mut());
        isr::enable_irqs();

        io::init(*PE_ID, "pemux");
    }

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

    // if the app exited, we re-enter env_run and send the exit request to the kernel
    if let Some(vpe) = CUR_VPE.get_mut().take() {
        send_exit(vpe);
    }

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
