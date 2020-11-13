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
use base::cfg::PE_COUNT;
use base::col::Vec;
use base::envdata;
use base::errors::Error;
use base::goff;
use base::kif::Perm;
use base::libc;
use base::rc::Rc;
use base::tcu::*;
use base::util;

use crate::ktcu;
use crate::pes::{PEMng, State, VPEMng, VPE};

pub fn rbuf_addrs(virt: goff) -> (goff, goff) {
    let off = virt - envdata::rbuf_start() as goff;
    (off, off)
}

pub fn deprivilege_pe(_pe: PEId) -> Result<(), Error> {
    // nothing to do
    Ok(())
}

pub fn reset_pe(_pe: PEId, pid: i32) -> Result<(), Error> {
    unsafe {
        libc::kill(pid, libc::SIGKILL);
    }
    Ok(())
}

pub fn config_recv(
    regs: &mut [Reg],
    _vpe: VPEId,
    buf: goff,
    buf_ord: u32,
    msg_ord: u32,
    _reply_eps: Option<EpId>,
) {
    regs[EpReg::VALID.val as usize] = 1;
    regs[EpReg::BUF_ADDR.val as usize] = buf as Reg;
    regs[EpReg::BUF_ORDER.val as usize] = buf_ord as Reg;
    regs[EpReg::BUF_MSGORDER.val as usize] = msg_ord as Reg;
    regs[EpReg::BUF_ROFF.val as usize] = 0;
    regs[EpReg::BUF_WOFF.val as usize] = 0;
    regs[EpReg::BUF_MSG_CNT.val as usize] = 0;
    regs[EpReg::BUF_UNREAD.val as usize] = 0;
    regs[EpReg::BUF_OCCUPIED.val as usize] = 0;
}

pub fn config_send(
    regs: &mut [Reg],
    _vpe: VPEId,
    lbl: Label,
    pe: PEId,
    dst_ep: EpId,
    msg_order: u32,
    credits: u32,
) {
    regs[EpReg::VALID.val as usize] = 1;
    regs[EpReg::LABEL.val as usize] = lbl;
    regs[EpReg::PE_ID.val as usize] = pe as Reg;
    regs[EpReg::EP_ID.val as usize] = dst_ep as Reg;
    if credits == UNLIM_CREDITS {
        regs[EpReg::CREDITS.val as usize] = credits as Reg;
    }
    else {
        regs[EpReg::CREDITS.val as usize] = ((1 << msg_order) * credits) as Reg;
    }
    regs[EpReg::MSGORDER.val as usize] = msg_order as Reg;
}

pub fn config_mem(regs: &mut [Reg], _vpe: VPEId, pe: PEId, addr: goff, size: usize, perm: Perm) {
    regs[EpReg::VALID.val as usize] = 1;
    regs[EpReg::LABEL.val as usize] = addr as Reg;
    regs[EpReg::PERM.val as usize] = perm.bits() as Reg;
    regs[EpReg::PE_ID.val as usize] = pe as Reg;
    regs[EpReg::EP_ID.val as usize] = 0;
    regs[EpReg::CREDITS.val as usize] = size as Reg;
    regs[EpReg::MSGORDER.val as usize] = 0;
}

pub fn invalidate_ep_remote(pe: PEId, ep: EpId, _force: bool) -> Result<u32, Error> {
    let regs = [0 as Reg; EP_REGS];
    write_ep_remote(pe, ep, &regs).map(|_| 0)
}

pub fn inv_reply_remote(
    _recv_pe: PEId,
    _recv_ep: EpId,
    _send_pe: PEId,
    _send_ep: EpId,
) -> Result<(), Error> {
    // nothing to do
    Ok(())
}

#[derive(Default)]
struct EP {
    regs: Vec<Reg>,
    dirty: bool,
}

impl EP {
    fn new(regs: &[Reg], dirty: bool) -> Self {
        Self {
            regs: regs.to_vec(),
            dirty,
        }
    }
}

static ALL_EPS: StaticCell<Vec<EP>> = StaticCell::new(Vec::new());

fn ep_idx(pe: PEId, ep: EpId) -> usize {
    pe as usize * EP_COUNT as usize + ep as usize
}

pub fn init() {
    for _ in 0..PE_COUNT {
        for _ in 0..EP_COUNT {
            ALL_EPS.get_mut().push(EP::new(&[0; EP_REGS], false));
        }
    }
}

pub fn write_ep_remote(pe: PEId, ep: EpId, regs: &[Reg]) -> Result<(), Error> {
    let vpe = VPEMng::get()
        .find_vpe(|v: &Rc<VPE>| v.pe_id() == pe)
        .unwrap();
    if vpe.state() == State::RUNNING {
        let eps = PEMng::get().pemux(pe).eps_base() as usize;
        let addr = eps + ep as usize * EP_REGS * util::size_of::<Reg>();
        let bytes = EP_REGS * util::size_of::<Reg>();
        ktcu::try_write_mem(pe, addr as goff, regs.as_ptr() as *const u8, bytes)
    }
    else {
        ALL_EPS.get_mut()[ep_idx(pe, ep)] = EP::new(regs, true);
        Ok(())
    }
}

pub fn update_eps(pe: PEId, base: goff) -> Result<(), Error> {
    for ep in FIRST_USER_EP..EP_COUNT {
        let mut ep_obj = &mut ALL_EPS.get_mut()[ep_idx(pe, ep)];
        if ep_obj.dirty {
            ep_obj.regs[EpReg::BUF_ADDR.val as usize] += base;
            write_ep_remote(pe, ep, &ep_obj.regs)?;
            ep_obj.dirty = false;
        }
    }
    Ok(())
}
