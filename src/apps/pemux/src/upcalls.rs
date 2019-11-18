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

use base::cfg;
use base::dtu;
use base::envdata;
use base::errors::{Code, Error};
use base::io;
use base::kif;
use base::util;
use core::intrinsics;

use helper;
use isr;
use vpe;

fn env() -> &'static mut envdata::EnvData {
    unsafe { intrinsics::transmute(cfg::ENV_START) }
}

fn reply_msg<T>(msg: &'static dtu::Message, reply: &T) {
    let _irqs = helper::IRQsOnGuard::new();
    dtu::DTU::reply(
        dtu::PEXUP_REP,
        reply as *const T as *const u8,
        util::size_of::<T>(),
        msg,
    )
    .unwrap();
}

fn vpe_ctrl(
    msg: &'static dtu::Message,
    state: &mut isr::State,
    vpe: &mut u64,
) -> Result<(), Error> {
    let req = msg.get_data::<kif::pemux::VPECtrl>();

    let pe_id = req.pe_id as u32;
    let vpe_id = req.vpe_sel;
    let op = kif::pemux::VPEOp::from(req.vpe_op);

    // do that here to get the color of the next print correct
    if op == kif::pemux::VPEOp::INIT {
        io::init(pe_id, "pemux");
    }

    log!(PEX_UPCALLS, "upcall::vpe_ctrl(vpe={}, op={:?})", vpe_id, op);

    match op {
        kif::pemux::VPEOp::INIT => {
            *vpe = vpe_id;
        },

        kif::pemux::VPEOp::START => {
            // remember the current PE
            env().pe_id = pe_id;
            state.init(env().entry as usize, env().sp as usize);
            vpe::add(vpe_id);
            *vpe = vpe_id;
        },

        _ => {
            state.stop();
            *vpe = kif::pemux::IDLE_ID;
        },
    }

    Ok(())
}

fn handle_upcall(msg: &'static dtu::Message, state: &mut isr::State, vpe: &mut u64) {
    let req = msg.get_data::<kif::DefaultRequest>();

    let res = match kif::pemux::Upcalls::from(req.opcode) {
        kif::pemux::Upcalls::VPE_CTRL => vpe_ctrl(msg, state, vpe),
        _ => Err(Error::new(Code::NotSup)),
    };

    match res {
        Ok(_) => reply_msg(msg, &kif::DefaultReply { error: 0 }),
        Err(e) => reply_msg(msg, &kif::DefaultReply {
            error: e.code() as u64,
        }),
    }
}

pub fn check(state: &mut isr::State) {
    let _guard = helper::DTUGuard::new();

    // change to our VPE
    let mut old_vpe = dtu::DTU::get_vpe_id();
    dtu::DTU::set_vpe_id(kif::pemux::VPE_ID);

    let msg = dtu::DTU::fetch_msg(dtu::PEXUP_REP);
    if let Some(m) = msg {
        handle_upcall(m, state, &mut old_vpe);
    }

    // change back to old VPE
    dtu::DTU::set_vpe_id(old_vpe);
}
