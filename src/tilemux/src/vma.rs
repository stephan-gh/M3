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
use base::errors::{Code, Error};
use base::io::LogFlags;
use base::kif::PageFlags;
use base::log;
use base::mem::{MsgBuf, VirtAddr};
use base::tcu;

use crate::activities;
use crate::helper;

use isr::StateArch;

pub struct PfState {
    virt: VirtAddr,
    perm: PageFlags,
}

fn send_pf(
    mut act: activities::ActivityRef<'_>,
    virt: VirtAddr,
    perm: PageFlags,
) -> Result<(), Error> {
    // save command registers to be able to send a message
    let _cmd_saved = helper::TCUGuard::new();

    // change to the activity, if required
    if act.state() != activities::ActState::Running {
        let mut cur = activities::cur();
        let old_act = tcu::TCU::xchg_activity(act.activity_reg()).unwrap();
        cur.set_activity_reg(old_act);
    }

    // build message
    let mut msg_buf = MsgBuf::borrow_def();
    msg_buf.set(crate::PagefaultMessage {
        op: 0, // PagerOp::PAGEFAULT
        virt,
        access: perm.bits(),
    });

    // send PF message
    let eps_start = act.eps_start();
    let res = tcu::TCU::send(
        eps_start + tcu::PG_SEP_OFF,
        &msg_buf,
        0,
        eps_start + tcu::PG_REP_OFF,
    )
    .map(|_| {
        // remember the page fault information to resume it later
        act.start_pf(PfState { virt, perm });
        act.block(
            Some(recv_pf_resp),
            Some(eps_start + tcu::PG_REP_OFF),
            None,
            None,
        );
    });

    if act.state() != activities::ActState::Running {
        let cur = activities::cur();
        act.set_activity_reg(tcu::TCU::xchg_activity(cur.activity_reg()).unwrap());
    }
    res
}

fn recv_pf_resp(cur: &mut activities::Activity) -> activities::ContResult {
    // save command registers to be able to send a message
    let _cmd_saved = helper::TCUGuard::new();

    let eps_start = cur.eps_start();

    if let Some(msg_off) = tcu::TCU::fetch_msg(eps_start + tcu::PG_REP_OFF) {
        let rbuf_space = crate::pex_env().tile_desc.rbuf_std_space();
        let rbuf_addr =
            rbuf_space.0 + cfg::SYSC_RBUF_SIZE + cfg::UPCALL_RBUF_SIZE + cfg::DEF_RBUF_SIZE;
        let msg = tcu::TCU::offset_to_msg(rbuf_addr, msg_off);
        let err = msg.as_words()[0] as u32;
        // deliberately ignore errors here; the kernel can invalidate the pager EPs at any time
        tcu::TCU::ack_msg(eps_start + tcu::PG_REP_OFF, msg_off).ok();

        let pf_state = cur.finish_pf();
        if err != 0 {
            log!(
                LogFlags::Error,
                "Pagefault for {} (perm: {:?}) with user state:\n{:?}",
                pf_state.virt,
                pf_state.perm,
                cur.user_state()
            );
            activities::ContResult::Failure
        }
        else {
            activities::ContResult::Success
        }
    }
    else {
        activities::ContResult::Waiting
    }
}

pub fn handle_xlate(virt: VirtAddr, perm: PageFlags) {
    // perform page table walk
    let act = activities::cur();
    let (phys, flags) = act.translate(virt, perm);

    // page fault?
    if (!(flags & PageFlags::RW) & perm) != PageFlags::empty() {
        // TODO directly insert into TLB when the PF was resolved?
        if send_pf(act, virt, perm).is_err() {
            log!(LogFlags::Error, "Unable to handle page fault for {}", virt);
            activities::remove_cur(Code::Unspecified);
        }
    }
    // translation worked: let transfer continue
    else {
        // ensure that we only insert user-accessible pages into the TLB
        if !flags.contains(PageFlags::U) {
            log!(LogFlags::Error, "No permission to access {}", virt);
            activities::remove_cur(Code::Unspecified);
        }
        else {
            tcu::TCU::insert_tlb(act.id() as u16, virt, phys, flags).unwrap();
        }
    }
}

pub fn handle_pf(state: &crate::arch::State, virt: VirtAddr, perm: PageFlags) -> Result<(), Error> {
    // TileMux isn't causing PFs
    if !state.came_from_user() {
        panic!("pagefault for {} at {}", virt, state.instr_pointer());
    }

    if let Err(e) = send_pf(activities::cur(), virt, perm) {
        log!(
            LogFlags::Error,
            "Pagefault for {} with user state:\n{:?}",
            virt,
            state
        );
        return Err(e);
    }

    Ok(())
}
