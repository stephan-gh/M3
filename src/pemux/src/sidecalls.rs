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
use base::mem::{GlobAddr, MsgBuf};
use base::tcu;

use crate::helper;
use crate::vpe;

const SIDE_RBUF_ADDR: usize = cfg::PEMUX_RBUF_SPACE + cfg::KPEX_RBUF_SIZE;

fn reply_msg(msg: &'static tcu::Message, reply: &MsgBuf) {
    let msg_off = tcu::TCU::msg_to_offset(SIDE_RBUF_ADDR, msg);
    tcu::TCU::reply(tcu::PEXSIDE_REP, reply, msg_off).unwrap();
}

fn vpe_ctrl(msg: &'static tcu::Message) -> Result<(), Error> {
    let req = msg.get_data::<kif::pemux::VPECtrl>();

    let vpe_id = req.vpe_sel as vpe::Id;
    let op = kif::pemux::VPEOp::from(req.vpe_op);
    let eps_start = req.eps_start as tcu::EpId;

    log!(
        crate::LOG_SIDECALLS,
        "sidecall::vpe_ctrl(vpe={}, op={:?}, eps_start={})",
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
            vpe.unblock(vpe::Event::Start);
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

    log!(
        crate::LOG_SIDECALLS,
        "sidecall::map(vpe={}, virt={:#x}, global={:?}, pages={}, perm={:?})",
        vpe_id,
        virt,
        global,
        pages,
        perm
    );

    // ensure that we don't overmap critical areas
    if virt < cfg::ENV_START || virt + pages * cfg::PAGE_SIZE > cfg::PE_MEM_BASE {
        return Err(Error::new(Code::InvArgs));
    }

    if let Some(vpe) = vpe::get_mut(vpe_id) {
        // if we unmap these pages, flush+invalidate the cache to ensure that we read this memory
        // fresh from DRAM the next time we use it.
        if (perm & kif::PageFlags::RWX).is_empty() {
            helper::flush_invalidate();
        }

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
        crate::LOG_SIDECALLS,
        "sidecall::translate(vpe={}, virt={:#x}, perm={:?})",
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
        Ok(GlobAddr::new_from_phys(pte).unwrap().raw())
    }
}

fn rem_msgs(msg: &'static tcu::Message) -> Result<(), Error> {
    let req = msg.get_data::<kif::pemux::RemMsgs>();

    let vpe_id = req.vpe_sel as vpe::Id;
    let unread = req.unread_mask as u32;

    log!(
        crate::LOG_SIDECALLS,
        "sidecall::rem_msgs(vpe={}, unread={})",
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
        crate::LOG_SIDECALLS,
        "sidecall::ep_inval(vpe={}, ep={})",
        vpe_id,
        ep
    );

    // just unblock the VPE in case it wants to do something on invalidated EPs
    if let Some(vpe) = vpe::get_mut(vpe_id) {
        vpe.unblock(vpe::Event::EpInvalid);
    }

    Ok(())
}

fn reset_stats(_msg: &'static tcu::Message) -> Result<(), Error> {
    log!(crate::LOG_SIDECALLS, "sidecall::reset_stats()",);

    for id in 0..64 {
        if let Some(vpe) = vpe::get_mut(id) {
            vpe.reset_stats();
        }
    }

    Ok(())
}

fn handle_sidecall(msg: &'static tcu::Message) {
    let req = msg.get_data::<kif::DefaultRequest>();

    let mut val = 0;
    let op = kif::pemux::Sidecalls::from(req.opcode);
    let res = match op {
        kif::pemux::Sidecalls::VPE_CTRL => vpe_ctrl(msg),
        kif::pemux::Sidecalls::MAP => map(msg),
        kif::pemux::Sidecalls::TRANSLATE => translate(msg).map(|pte| val = pte),
        kif::pemux::Sidecalls::REM_MSGS => rem_msgs(msg),
        kif::pemux::Sidecalls::EP_INVAL => ep_inval(msg),
        kif::pemux::Sidecalls::RESET_STATS => reset_stats(msg),
        _ => Err(Error::new(Code::NotSup)),
    };

    let mut reply_buf = MsgBuf::borrow_def();
    reply_buf.set(kif::pemux::Response {
        error: match res {
            Ok(_) => 0,
            Err(e) => {
                log!(crate::LOG_SIDECALLS, "sidecall {} failed: {}", op, e);
                e.code() as u64
            },
        },
        val,
    });
    reply_msg(msg, &reply_buf);
}

#[inline(never)]
fn handle_sidecalls(our: &mut vpe::VPE) {
    let _cmd_saved = helper::TCUGuard::new();

    loop {
        // change to our VPE
        let old_vpe = tcu::TCU::xchg_vpe(our.vpe_reg()).unwrap();
        if let Some(old) = vpe::try_cur() {
            old.set_vpe_reg(old_vpe);
        }

        if let Some(msg_off) = tcu::TCU::fetch_msg(tcu::PEXSIDE_REP) {
            let msg = tcu::TCU::offset_to_msg(SIDE_RBUF_ADDR, msg_off);
            handle_sidecall(msg);
        }

        // just ACK replies from the kernel; we don't care about them
        if let Some(msg_off) = tcu::TCU::fetch_msg(tcu::KPEX_REP) {
            tcu::TCU::ack_msg(tcu::KPEX_REP, msg_off).unwrap();
        }

        // change back to old VPE
        let new_vpe = vpe::try_cur().map_or(old_vpe, |new| new.vpe_reg());
        our.set_vpe_reg(tcu::TCU::xchg_vpe(new_vpe).unwrap());
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

    handle_sidecalls(our);
}
