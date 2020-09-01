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
#![no_std]

extern crate base;

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
pub const LOG_UPCALLS: bool = true;

#[no_mangle]
pub extern "C" fn abort() -> ! {
    exit(1);
}

#[no_mangle]
pub extern "C" fn exit(_code: i32) -> ! {
    machine::shutdown();
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

fn vpe_ctrl(msg: &'static tcu::Message) -> Result<(), Error> {
    let req = msg.get_data::<kif::pemux::VPECtrl>();

    let vpe_id = req.vpe_sel;
    let op = kif::pemux::VPEOp::from(req.vpe_op);
    let eps_start = req.eps_start as tcu::EpId;

    log!(
        crate::LOG_UPCALLS,
        "upcall::vpe_ctrl(vpe={}, op={:?}, eps_start={})",
        vpe_id,
        op,
        eps_start
    );

    // match op {
    //     kif::pemux::VPEOp::INIT => {
    //         vpe::add(vpe_id, eps_start);
    //     },

    //     kif::pemux::VPEOp::START => {
    //         let cur = vpe::cur();
    //         let vpe = vpe::get_mut(vpe_id).unwrap();
    //         assert!(cur.id() != vpe.id());
    //         // temporary switch to the VPE to access the environment
    //         vpe.switch_to();
    //         vpe.start();
    //         vpe.unblock(None, false);
    //         // now switch back
    //         cur.switch_to();
    //     },

    //     _ => {
    //         // we cannot remove the current VPE here; remove it via scheduling
    //         match vpe::try_cur() {
    //             Some(cur) if cur.id() == vpe_id => crate::reg_scheduling(vpe::ScheduleAction::Kill),
    //             _ => vpe::remove(vpe_id, 0, false, true),
    //         }
    //     },
    // }

    Ok(())
}

fn handle_upcall(msg: &'static tcu::Message) {
    let req = msg.get_data::<kif::DefaultRequest>();
    let opcode = kif::pemux::Upcalls::from(req.opcode);

    log!(
        crate::LOG_UPCALLS,
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
        Err(e) => e.code() as u64,
    };
    reply_msg(msg, &reply);
}

#[no_mangle]
pub extern "C" fn env_run() {
    io::init(envdata::get().pe_id, "pemux");
    log!(crate::LOG_UPCALLS, "Hello World!");

    isr::init(cfg::STACK_BOTTOM + cfg::STACK_SIZE / 2);
    isr::enable_irqs();

    // wait until the kernel configured the EP
    if envdata::get().platform == envdata::Platform::GEM5.val {
        loop {
            if tcu::TCU::is_valid(tcu::PEXUP_REP) {
                break;
            }
        }
    }

    loop {
        if envdata::get().platform == envdata::Platform::GEM5.val {
            tcu::TCU::sleep().ok();
        }

        if let Some(msg_off) = tcu::TCU::fetch_msg(tcu::PEXUP_REP) {
            let msg = tcu::TCU::offset_to_msg(UPC_RBUF_ADDR, msg_off);
            handle_upcall(msg);
        }
    }
}
