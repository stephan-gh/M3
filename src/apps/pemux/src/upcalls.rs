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

use base::cell::StaticCell;
use base::cfg;
use base::dtu;
use base::errors::{Code, Error};
use base::goff;
use base::io;
use base::kif;
use base::util;

use arch;
use helper;
use vpe;

static ENABLED: StaticCell<bool> = StaticCell::new(true);

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

fn vpe_ctrl(msg: &'static dtu::Message, state: &mut arch::State) -> Result<(), Error> {
    let req = msg.get_data::<kif::pemux::VPECtrl>();

    let pe_id = req.pe_id as u32;
    let vpe_id = req.vpe_sel;
    let op = kif::pemux::VPEOp::from(req.vpe_op);

    if op == kif::pemux::VPEOp::INIT {
        // do that here to get the color of the next print correct
        io::init(pe_id, "pemux");
    }

    log!(
        crate::LOG_UPCALLS,
        "upcall::vpe_ctrl(vpe={}, op={:?})",
        vpe_id,
        op
    );

    match op {
        kif::pemux::VPEOp::INIT => {
            vpe::add(vpe_id);
        },

        kif::pemux::VPEOp::START => {
            // remember the current PE
            ::env().pe_id = pe_id;
            state.init(::env().entry as usize, ::env().sp as usize);
        },

        kif::pemux::VPEOp::STOP | _ => {
            crate::stop_vpe(state);
            vpe::remove(0, false);
        },
    }

    Ok(())
}

fn map(msg: &'static dtu::Message) -> Result<(), Error> {
    let req = msg.get_data::<kif::pemux::Map>();

    let vpe_id = req.vpe_sel;
    let virt = req.virt as usize;
    let phys = req.phys as goff;
    let pages = req.pages as usize;
    let perm = kif::PageFlags::from_bits_truncate(req.perm as u64);

    // ensure that we don't overmap critical areas
    if virt < cfg::ENV_START || virt + pages * cfg::PAGE_SIZE > cfg::RECVBUF_SPACE {
        return Err(Error::new(Code::InvArgs));
    }

    log!(
        crate::LOG_UPCALLS,
        "upcall::map(vpe={}, virt={:#x}, phys={:#x}, pages={}, perm={:?})",
        vpe_id,
        virt,
        phys,
        pages,
        perm
    );

    vpe::get_mut(vpe_id)
        .unwrap()
        .map(virt, phys, pages, perm | kif::PageFlags::U)
}

fn rem_msgs(msg: &'static dtu::Message) -> Result<(), Error> {
    let req = msg.get_data::<kif::pemux::RemMsgs>();

    let vpe_id = req.vpe_sel;
    let unread = req.unread_mask as u32;

    log!(
        crate::LOG_UPCALLS,
        "upcall::rem_msgs(vpe={}, unread={})",
        vpe_id,
        unread
    );

    // we know that this VPE is not currently running, because we changed the current VPE to ourself
    // in check() below.
    vpe::get_mut(vpe_id)
        .unwrap()
        .rem_msgs(unread.count_ones() as u16);

    Ok(())
}

fn handle_upcall(msg: &'static dtu::Message, state: &mut arch::State) {
    let req = msg.get_data::<kif::DefaultRequest>();

    let res = match kif::pemux::Upcalls::from(req.opcode) {
        kif::pemux::Upcalls::VPE_CTRL => vpe_ctrl(msg, state),
        kif::pemux::Upcalls::MAP => map(msg),
        kif::pemux::Upcalls::REM_MSGS => rem_msgs(msg),
        _ => Err(Error::new(Code::NotSup)),
    };

    match res {
        Ok(_) => reply_msg(msg, &kif::DefaultReply { error: 0 }),
        Err(e) => reply_msg(msg, &kif::DefaultReply {
            error: e.code() as u64,
        }),
    }
}

pub fn disable() -> bool {
    ENABLED.set(false)
}

pub fn enable() {
    ENABLED.set(true);
}

pub fn check(state: &mut arch::State) {
    if !*ENABLED {
        return;
    }

    let our = vpe::our();
    if !our.has_msgs() {
        return;
    }

    let _cmd_saved = helper::DTUGuard::new();

    // don't handle other upcalls in the meantime
    let _upcalls_off = helper::UpcallsOffGuard::new();

    loop {
        // change to our VPE
        let old_vpe = dtu::DTU::xchg_vpe(our.vpe_reg());
        vpe::cur().set_vpe_reg(old_vpe);

        let msg = dtu::DTU::fetch_msg(dtu::PEXUP_REP);
        if let Some(m) = msg {
            handle_upcall(m, state);
        }

        // change back to old VPE
        let new_vpe = vpe::cur().vpe_reg();
        our.set_vpe_reg(dtu::DTU::xchg_vpe(new_vpe));
        // if no events arrived in the meantime, we're done
        if !our.has_msgs() {
            break;
        }
    }
}
