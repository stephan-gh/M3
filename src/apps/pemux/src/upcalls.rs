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
use base::errors::{Code, Error};
use base::goff;
use base::kif;
use base::tcu;
use base::util;

use helper;
use vpe;

fn reply_msg<T>(msg: &'static tcu::Message, reply: &T) {
    tcu::TCU::reply(
        tcu::PEXUP_REP,
        reply as *const T as *const u8,
        util::size_of::<T>(),
        msg,
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
        "upcall::vpe_ctrl(vpe={}, op={:?})",
        vpe_id,
        op
    );

    match op {
        kif::pemux::VPEOp::INIT => {
            vpe::add(vpe_id, eps_start);
        },

        kif::pemux::VPEOp::START => {
            let cur = vpe::cur();
            let vpe = vpe::get_mut(vpe_id).unwrap();
            assert!(cur.id() != vpe.id());
            // temporary switch to the VPE to access the environment
            vpe.switch_to();
            vpe.start();
            vpe.unblock(None);
            // now switch back
            cur.switch_to();
        },

        kif::pemux::VPEOp::STOP | _ => {
            // we cannot remove the current VPE here; remove it via scheduling
            if vpe::cur().id() == vpe_id {
                crate::reg_scheduling(vpe::ScheduleAction::Kill);
            }
            else {
                vpe::remove(vpe_id, 0, false, true);
            }
        },
    }

    Ok(())
}

fn map(msg: &'static tcu::Message) -> Result<(), Error> {
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

fn rem_msgs(msg: &'static tcu::Message) -> Result<(), Error> {
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

fn ep_inval(msg: &'static tcu::Message) -> Result<(), Error> {
    let req = msg.get_data::<kif::pemux::EpInval>();

    let vpe_id = req.vpe_sel;
    let ep = req.ep as tcu::EpId;

    log!(
        crate::LOG_UPCALLS,
        "upcall::ep_inval(vpe={}, ep={})",
        vpe_id,
        ep
    );

    // just unblock the VPE in case it wants to do something on invalidated EPs
    vpe::get_mut(vpe_id).unwrap().unblock(None);

    Ok(())
}

fn handle_upcall(msg: &'static tcu::Message) {
    let req = msg.get_data::<kif::DefaultRequest>();

    let res = match kif::pemux::Upcalls::from(req.opcode) {
        kif::pemux::Upcalls::VPE_CTRL => vpe_ctrl(msg),
        kif::pemux::Upcalls::MAP => map(msg),
        kif::pemux::Upcalls::REM_MSGS => rem_msgs(msg),
        kif::pemux::Upcalls::EP_INVAL => ep_inval(msg),
        _ => Err(Error::new(Code::NotSup)),
    };

    let reply = &mut crate::msgs_mut().upcall_reply;
    reply.error = match res {
        Ok(_) => 0,
        Err(e) => e.code() as u64,
    };
    reply_msg(msg, reply);
}

#[inline(never)]
fn handle_upcalls(our: &mut vpe::VPE) {
    let _cmd_saved = helper::TCUGuard::new();

    loop {
        // change to our VPE
        let old_vpe = tcu::TCU::xchg_vpe(our.vpe_reg());
        vpe::cur().set_vpe_reg(old_vpe);

        if let Some(m) = tcu::TCU::fetch_msg(tcu::PEXUP_REP) {
            handle_upcall(m);
        }

        // just ACK replies from the kernel; we don't care about them
        if let Some(m) = tcu::TCU::fetch_msg(tcu::KPEX_REP) {
            tcu::TCU::ack_msg(tcu::KPEX_REP, &m);
        }

        // change back to old VPE
        let new_vpe = vpe::cur().vpe_reg();
        our.set_vpe_reg(tcu::TCU::xchg_vpe(new_vpe));
        // if no events arrived in the meantime, we're done
        if !our.has_msgs() {
            break;
        }
    }
}

#[inline(always)]
pub fn check() {
    let our = vpe::our();
    if !our.has_msgs() {
        return;
    }

    handle_upcalls(our);
}
