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
use base::tcu;
use base::util;

use helper;
use vpe;

pub struct PfState {
    buf: Option<u64>,
    virt: usize,
    perm: PageFlags,
}

fn send_pf(
    vpe: &mut vpe::VPE,
    mut buf: Option<u64>,
    virt: usize,
    perm: PageFlags,
) -> Result<(), Error> {
    // save command registers to be able to send a message
    let cmd_saved = helper::TCUGuard::new();

    // if the command triggered the page fault, the core request has been aborted and we don't want
    // to resume it later
    if let Some(buf_id) = buf {
        if cmd_saved.state().xfer_buf() == buf_id {
            buf = None;
        }
    }

    // change to the VPE, if required
    let cur = vpe::cur();
    if cur.id() != vpe.id() {
        let old_vpe = tcu::TCU::xchg_vpe(vpe.vpe_reg());
        cur.set_vpe_reg(old_vpe);
    }

    // build message
    let msg = &mut crate::msgs_mut().pagefault;
    msg.op = 0;
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
    ).and_then(|_| {
        // remember the page fault information to resume it later
        vpe.start_pf(PfState { buf, virt, perm });
        vpe.block(vpe::ScheduleAction::Block, Some(recv_pf_resp));
        Ok(())
    });

    if cur.id() != vpe.id() {
        vpe.set_vpe_reg(tcu::TCU::xchg_vpe(cur.vpe_reg()));
    }
    res
}

fn recv_pf_resp() -> bool {
    let vpe = vpe::cur();
    let eps_start = vpe.eps_start();

    if let Some(msg) = tcu::TCU::fetch_msg(eps_start + tcu::PG_REP_OFF) {
        let reply = msg.get_data::<DefaultReply>();
        let err = reply.error as u32;
        tcu::TCU::ack_msg(eps_start + tcu::PG_REP_OFF, msg);

        let pf_state = vpe.finish_pf();
        if let Some(buf) = pf_state.buf {
            let pte = if err == 0 {
                vpe.translate(pf_state.virt, pf_state.perm)
            }
            else {
                cfg::PAGE_SIZE as u64
            };
            tcu::TCU::set_core_resp(pte | (buf << 6));
        }
        if err != 0 {
            vpe::remove_cur(1);
        }
        true
    }
    else {
        false
    }
}

pub fn handle_xlate(req: tcu::Reg) {
    let asid = req >> 48;
    let virt = ((req & 0xFFFF_FFFF_FFFF) as usize) & !cfg::PAGE_MASK as usize;
    let perm = PageFlags::from_bits_truncate((req >> 1) & PageFlags::RW.bits());
    let xfer_buf = (req >> 6) & 0x7;

    // perform page table walk
    let vpe = vpe::get_mut(asid);
    if let Some(vpe) = vpe {
        let pte = vpe.translate(virt, perm);
        // page fault?
        if (!(pte & PageFlags::RW.bits()) & perm.bits()) != 0 {
            // the first xfer buffer can't raise pagefaults
            if xfer_buf != 0 {
                if send_pf(vpe, Some(xfer_buf), virt, perm).is_ok() {
                    return;
                }
            }
        }
        // translation worked: let transfer continue
        else {
            tcu::TCU::set_core_resp(pte | (xfer_buf << 6));
            return;
        }
    }

    // translation failed: set non-zero response, but have no permission bits set
    tcu::TCU::set_core_resp(cfg::PAGE_SIZE as u64 | (xfer_buf << 6));
}

pub fn handle_pf(
    state: &crate::arch::State,
    virt: usize,
    perm: PageFlags,
    ip: usize,
) -> Result<(), Error> {
    // PEMux isn't causing PFs
    if !state.came_from_user() {
        // save the current command to ensure that we can use the print command
        let _cmd_saved = helper::TCUGuard::new();
        panic!("pagefault for {:#x} at {:#x}", virt, ip);
    }

    if let Err(e) = send_pf(vpe::cur(), None, virt, perm) {
        log!(crate::LOG_ERR, "Pagefault for {:#x} with {:?}", virt, state);
        return Err(e);
    }

    Ok(())
}
