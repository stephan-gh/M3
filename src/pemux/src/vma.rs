/*
 * Copyright (C) 2015, René Küttner <rene.kuettner@.tu-dresden.de>
 * Economic rights: Technische Universität Dresden (Germany)
 *
 * This file is part of M3 (Microkernel for Minimalist Manycores).
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
use base::errors::Error;
use base::kif::{DefaultReply, PageFlags};
use base::log;
use base::tcu;
use base::util;

use crate::helper;
use crate::vpe;

pub struct PfState {
    virt: usize,
    perm: PageFlags,
}

fn send_pf(vpe: &mut vpe::VPE, virt: usize, perm: PageFlags) -> Result<(), Error> {
    // save command registers to be able to send a message
    let _cmd_saved = helper::TCUGuard::new();

    // change to the VPE, if required
    let cur = vpe::cur();
    if cur.id() != vpe.id() {
        let old_vpe = tcu::TCU::xchg_vpe(vpe.vpe_reg());
        cur.set_vpe_reg(old_vpe);
    }

    // build message
    let msg = &mut crate::msgs_mut().pagefault;
    msg.op = 0; // PagerOp::PAGEFAULT
    msg.virt = virt as u64;
    msg.access = perm.bits();

    // send PF message
    let eps_start = vpe.eps_start();
    let res = tcu::TCU::send(
        eps_start + tcu::PG_SEP_OFF,
        msg as *const _ as *const u8,
        util::size_of::<crate::PagefaultMessage>(),
        0,
        eps_start + tcu::PG_REP_OFF,
    )
    .map(|_| {
        // remember the page fault information to resume it later
        vpe.start_pf(PfState { virt, perm });
        vpe.block(Some(recv_pf_resp), Some(eps_start + tcu::PG_REP_OFF), None);
    });

    if cur.id() != vpe.id() {
        vpe.set_vpe_reg(tcu::TCU::xchg_vpe(cur.vpe_reg()));
    }
    res
}

fn recv_pf_resp() -> vpe::ContResult {
    // save command registers to be able to send a message
    let _cmd_saved = helper::TCUGuard::new();

    let vpe = vpe::cur();
    let eps_start = vpe.eps_start();

    if let Some(msg_off) = tcu::TCU::fetch_msg(eps_start + tcu::PG_REP_OFF) {
        let rbuf_space = crate::pex_env().pe_desc.rbuf_std_space();
        let rbuf_addr =
            rbuf_space.0 + cfg::SYSC_RBUF_SIZE + cfg::UPCALL_RBUF_SIZE + cfg::DEF_RBUF_SIZE;
        let msg = tcu::TCU::offset_to_msg(rbuf_addr, msg_off);
        let reply = msg.get_data::<DefaultReply>();
        let err = reply.error as u32;
        // deliberately ignore errors here; the kernel can invalidate the pager EPs at any time
        tcu::TCU::ack_msg(eps_start + tcu::PG_REP_OFF, msg_off).ok();

        let pf_state = vpe.finish_pf();
        if err != 0 {
            log!(
                crate::LOG_ERR,
                "Pagefault for {:#x} (perm: {:?}) with {:?}",
                pf_state.virt,
                pf_state.perm,
                vpe.user_state()
            );
            vpe::ContResult::Failure
        }
        else {
            vpe::ContResult::Success
        }
    }
    else {
        vpe::ContResult::Waiting
    }
}

pub fn handle_xlate(req: tcu::Reg) {
    let asid = req >> 48;
    let virt = ((req & 0xFFFF_FFFF_FFFF) as usize) & !cfg::PAGE_MASK as usize;
    let can_pf = ((req >> 1) & 0x1) != 0;
    let perm = PageFlags::from_bits_truncate((req >> 2) & PageFlags::RW.bits());

    // perform page table walk
    let vpe = vpe::get_mut(asid);
    if let Some(vpe) = vpe {
        let pte = vpe.translate(virt, perm);
        // page fault?
        if (!(pte & PageFlags::RW.bits()) & perm.bits()) != 0 {
            if can_pf && send_pf(vpe, virt, perm).is_ok() {
                return;
            }
        }
        // translation worked: let transfer continue
        else {
            tcu::TCU::set_core_req(pte);
            return;
        }
    }

    // translation failed: set non-zero response, but have no permission bits set
    tcu::TCU::set_core_req(cfg::PAGE_SIZE as u64);
}

pub fn handle_pf(
    state: &crate::arch::State,
    virt: usize,
    perm: PageFlags,
    ip: usize,
) -> Result<(), Error> {
    // PEMux isn't causing PFs
    if !state.came_from_user() {
        panic!("pagefault for {:#x} at {:#x}", virt, ip);
    }

    if let Err(e) = send_pf(vpe::cur(), virt, perm) {
        log!(crate::LOG_ERR, "Pagefault for {:#x} with {:?}", virt, state);
        return Err(e);
    }

    Ok(())
}
