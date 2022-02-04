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

use base::cell::StaticRefCell;
use base::cfg::TILE_COUNT;
use base::col::Vec;
use base::envdata;
use base::errors::Error;
use base::goff;
use base::kif::{PageFlags, Perm};
use base::libc;
use base::mem::{size_of, GlobAddr};
use base::rc::Rc;
use base::tcu::*;

use crate::ktcu;
use crate::tiles::{Activity, ActivityMng, State};

pub fn rbuf_addrs(virt: goff) -> (goff, goff) {
    let off = virt - envdata::rbuf_start() as goff;
    (off, off)
}

pub fn deprivilege_tile(_tile: TileId) -> Result<(), Error> {
    // nothing to do
    Ok(())
}

pub fn reset_tile(_tile: TileId, pid: i32) -> Result<(), Error> {
    unsafe {
        libc::kill(pid, libc::SIGKILL);
    }
    Ok(())
}

pub fn config_recv(
    regs: &mut [Reg],
    _act: ActId,
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
    _act: ActId,
    lbl: Label,
    tile: TileId,
    dst_ep: EpId,
    msg_order: u32,
    credits: u32,
) {
    regs[EpReg::VALID.val as usize] = 1;
    regs[EpReg::LABEL.val as usize] = lbl;
    regs[EpReg::TILE_ID.val as usize] = tile as Reg;
    regs[EpReg::EP_ID.val as usize] = dst_ep as Reg;
    if credits == UNLIM_CREDITS {
        regs[EpReg::CREDITS.val as usize] = credits as Reg;
    }
    else {
        regs[EpReg::CREDITS.val as usize] = ((1 << msg_order) * credits) as Reg;
    }
    regs[EpReg::MSGORDER.val as usize] = msg_order as Reg;
}

pub fn config_mem(
    regs: &mut [Reg],
    _act: ActId,
    tile: TileId,
    addr: goff,
    size: usize,
    perm: Perm,
) {
    regs[EpReg::VALID.val as usize] = 1;
    regs[EpReg::LABEL.val as usize] = addr as Reg;
    regs[EpReg::PERM.val as usize] = perm.bits() as Reg;
    regs[EpReg::TILE_ID.val as usize] = tile as Reg;
    regs[EpReg::EP_ID.val as usize] = 0;
    regs[EpReg::CREDITS.val as usize] = size as Reg;
    regs[EpReg::MSGORDER.val as usize] = 0;
}

pub fn invalidate_ep_remote(tile: TileId, ep: EpId, _force: bool) -> Result<u32, Error> {
    let regs = [0 as Reg; EP_REGS];
    write_ep_remote(tile, ep, &regs).map(|_| 0)
}

pub fn inv_reply_remote(
    _recv_tile: TileId,
    _recv_ep: EpId,
    _send_tile: TileId,
    _send_ep: EpId,
) -> Result<(), Error> {
    // nothing to do
    Ok(())
}

pub fn glob_to_phys_remote(
    _tile: TileId,
    glob: GlobAddr,
    _flags: PageFlags,
) -> Result<goff, Error> {
    Ok(glob.raw())
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

static ALL_EPS: StaticRefCell<Vec<EP>> = StaticRefCell::new(Vec::new());
static MEM_BASE: StaticRefCell<[usize; TILE_COUNT]> = StaticRefCell::new([0; TILE_COUNT]);

fn ep_idx(tile: TileId, ep: EpId) -> usize {
    tile as usize * TOTAL_EPS as usize + ep as usize
}

pub fn init() {
    let mut all_eps = ALL_EPS.borrow_mut();
    for _ in 0..TILE_COUNT {
        for _ in 0..TOTAL_EPS {
            all_eps.push(EP::new(&[0; EP_REGS], false));
        }
    }
}

pub fn set_mem_base(tile: TileId, base: usize) {
    MEM_BASE.borrow_mut()[tile as usize] = base;
}

pub fn write_ep_remote(tile: TileId, ep: EpId, regs: &[Reg]) -> Result<(), Error> {
    let act = ActivityMng::get()
        .find_activity(|v: &Rc<Activity>| v.tile_id() == tile)
        .unwrap();
    if act.state() == State::RUNNING {
        let eps = MEM_BASE.borrow()[tile as usize] as usize;
        let addr = eps + ep as usize * EP_REGS * size_of::<Reg>();
        let bytes = EP_REGS * size_of::<Reg>();
        ktcu::try_write_mem(tile, addr as goff, regs.as_ptr() as *const u8, bytes)
    }
    else {
        ALL_EPS.borrow_mut()[ep_idx(tile, ep)] = EP::new(regs, true);
        Ok(())
    }
}

pub fn update_eps(tile: TileId) -> Result<(), Error> {
    let base = MEM_BASE.borrow()[tile as usize];
    let mut all_eps = ALL_EPS.borrow_mut();
    for ep in FIRST_USER_EP..TOTAL_EPS {
        let mut ep_obj = &mut all_eps[ep_idx(tile, ep)];
        if ep_obj.dirty {
            ep_obj.regs[EpReg::BUF_ADDR.val as usize] += base as goff;
            write_ep_remote(tile, ep, &ep_obj.regs)?;
            ep_obj.dirty = false;
        }
    }
    Ok(())
}
