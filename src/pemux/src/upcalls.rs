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
use base::kif;
use base::log;
use base::mem::GlobAddr;
use base::tcu;
use base::util;

use crate::helper;
use crate::vpe;

const UPC_RBUF_ADDR: usize = cfg::PEMUX_RBUF_SPACE + cfg::KPEX_RBUF_SIZE;

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

    let vpe_id = req.vpe_sel as vpe::Id;
    let op = kif::pemux::VPEOp::from(req.vpe_op);
    let eps_start = req.eps_start as tcu::EpId;

    log!(
        crate::LOG_UPCALLS,
        "upcall::vpe_ctrl(vpe={}, op={:?}, eps_start={})",
        vpe_id,
        op,
        eps_start
    );

    match op {
        kif::pemux::VPEOp::INIT => vpe::add(vpe_id, eps_start),

        kif::pemux::VPEOp::START => {
            let cur = vpe::cur();
            let vpe = vpe::get_mut(vpe_id).unwrap();
            assert!(cur.id() != vpe.id());
            // temporary switch to the VPE to access the environment
            vpe.switch_to();
            vpe.start();
            vpe.unblock(None, false);
            // now switch back
            cur.switch_to();
            Ok(())
        },

        _ => {
            // we cannot remove the current VPE here; remove it via scheduling
            match vpe::try_cur() {
                Some(cur) if cur.id() == vpe_id => crate::reg_scheduling(vpe::ScheduleAction::Kill),
                _ => vpe::remove(vpe_id, 0, false, true),
            }
            Ok(())
        },
    }
}

fn map(msg: &'static tcu::Message) -> Result<(), Error> {
    let req = msg.get_data::<kif::pemux::Map>();

    let vpe_id = req.vpe_sel as vpe::Id;
    let virt = req.virt as usize;
    let global = GlobAddr::new(req.global);
    let pages = req.pages as usize;
    let perm = kif::PageFlags::from_bits_truncate(req.perm as u64);

    // ensure that we don't overmap critical areas
    if virt < cfg::ENV_START || virt + pages * cfg::PAGE_SIZE > cfg::PE_MEM_BASE {
        return Err(Error::new(Code::InvArgs));
    }

    log!(
        crate::LOG_UPCALLS,
        "upcall::map(vpe={}, virt={:#x}, global={:?}, pages={}, perm={:?})",
        vpe_id,
        virt,
        global,
        pages,
        perm
    );

    if let Some(vpe) = vpe::get_mut(vpe_id) {
        vpe.map(virt, global, pages, perm | kif::PageFlags::U)
    }
    else {
        Ok(())
    }
}

fn translate(msg: &'static tcu::Message) -> Result<kif::PTE, Error> {
    let req = msg.get_data::<kif::pemux::Translate>();

    let vpe_id = req.vpe_sel as vpe::Id;
    let virt = req.virt as usize;
    let perm = kif::PageFlags::from_bits_truncate(req.perm as u64);

    log!(
        crate::LOG_UPCALLS,
        "upcall::translate(vpe={}, virt={:#x}, perm={:?})",
        vpe_id,
        virt,
        perm
    );

    let pte = vpe::get_mut(vpe_id)
        .unwrap()
        .translate(virt, perm | kif::PageFlags::U);
    if (pte & perm.bits()) == 0 {
        Err(Error::new(Code::NoPerm))
    }
    else {
        Ok(paging::phys_to_glob(pte).unwrap().raw())
    }
}

fn rem_msgs(msg: &'static tcu::Message) -> Result<(), Error> {
    let req = msg.get_data::<kif::pemux::RemMsgs>();

    let vpe_id = req.vpe_sel as vpe::Id;
    let unread = req.unread_mask as u32;

    log!(
        crate::LOG_UPCALLS,
        "upcall::rem_msgs(vpe={}, unread={})",
        vpe_id,
        unread
    );

    // we know that this VPE is not currently running, because we changed the current VPE to ourself
    // in check() below.
    if let Some(vpe) = vpe::get_mut(vpe_id) {
        vpe.rem_msgs(unread.count_ones() as u16);
    }

    Ok(())
}

fn ep_inval(msg: &'static tcu::Message) -> Result<(), Error> {
    let req = msg.get_data::<kif::pemux::EpInval>();

    let vpe_id = req.vpe_sel as vpe::Id;
    let ep = req.ep as tcu::EpId;

    log!(
        crate::LOG_UPCALLS,
        "upcall::ep_inval(vpe={}, ep={})",
        vpe_id,
        ep
    );

    // just unblock the VPE in case it wants to do something on invalidated EPs
    if let Some(vpe) = vpe::get_mut(vpe_id) {
        vpe.unblock(None, false);
    }

    Ok(())
}

fn handle_upcall(msg: &'static tcu::Message) {
    let req = msg.get_data::<kif::DefaultRequest>();

    let reply = &mut crate::msgs_mut().upcall_reply;
    reply.val = 0;

    let res = match kif::pemux::Upcalls::from(req.opcode) {
        kif::pemux::Upcalls::VPE_CTRL => vpe_ctrl(msg),
        kif::pemux::Upcalls::MAP => map(msg),
        kif::pemux::Upcalls::TRANSLATE => translate(msg).map(|pte| reply.val = pte),
        kif::pemux::Upcalls::REM_MSGS => rem_msgs(msg),
        kif::pemux::Upcalls::EP_INVAL => ep_inval(msg),
        _ => Err(Error::new(Code::NotSup)),
    };

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
        if let Some(old) = vpe::try_cur() {
            old.set_vpe_reg(old_vpe);
        }

        if let Some(msg_off) = tcu::TCU::fetch_msg(tcu::PEXUP_REP) {
            let msg = tcu::TCU::offset_to_msg(UPC_RBUF_ADDR, msg_off);
            handle_upcall(msg);
        }

        // just ACK replies from the kernel; we don't care about them
        if let Some(msg_off) = tcu::TCU::fetch_msg(tcu::KPEX_REP) {
            tcu::TCU::ack_msg(tcu::KPEX_REP, msg_off).unwrap();
        }

        // change back to old VPE
        let new_vpe = vpe::try_cur().map_or(old_vpe, |new| new.vpe_reg());
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
