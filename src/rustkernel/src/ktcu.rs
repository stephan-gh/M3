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
use base::errors::{Code, Error};
use base::goff;
use base::kif;
use base::libc;
use base::math;
use base::tcu::{EpId, Header, Label, Message, PEId, Reg, EP_COUNT, EP_REGS, TCU};
use base::util;
use core::cmp;
use core::mem::MaybeUninit;

use pes::KERNEL_ID;

pub use arch::ktcu::*;

pub const KSYS_EP: EpId = 0;
pub const KSRV_EP: EpId = 1;
pub const KTMP_EP: EpId = 2;
pub const KPEX_EP: EpId = 3;

static BUF: StaticCell<[u8; 8192]> = StaticCell::new([0u8; 8192]);
static RBUFS: StaticCell<[usize; 8]> = StaticCell::new([0usize; 8]);

pub fn config_local_ep<CFG>(ep: EpId, cfg: CFG)
where
    CFG: FnOnce(&mut [Reg]),
{
    let mut regs = [0 as Reg; EP_REGS];
    cfg(&mut regs);
    TCU::set_ep_regs(ep, &regs);
}

pub fn config_remote_ep<CFG>(pe: PEId, ep: EpId, cfg: CFG) -> Result<(), Error>
where
    CFG: FnOnce(&mut [Reg]),
{
    let mut regs = [0 as Reg; EP_REGS];
    cfg(&mut regs);
    write_ep_remote(pe, ep, &regs)
}

pub fn recv_msgs(ep: EpId, buf: goff, ord: u32, msg_ord: u32) -> Result<(), Error> {
    static REPS: StaticCell<EpId> = StaticCell::new(16);

    if *REPS + (1 << (ord - msg_ord)) > EP_COUNT {
        return Err(Error::new(Code::NoSpace));
    }

    let (buf, phys) = rbuf_addrs(buf);
    config_local_ep(ep, |regs| {
        config_recv(regs, KERNEL_ID, phys, ord, msg_ord, Some(*REPS));
        *REPS.get_mut() += 1 << (ord - msg_ord);
    });
    RBUFS.get_mut()[ep] = buf as usize;
    Ok(())
}

pub fn reply<R>(ep: EpId, reply: &R, msg: &Message) -> Result<(), Error> {
    let msg_off = TCU::msg_to_offset(RBUFS[ep], msg);
    TCU::reply(
        ep,
        reply as *const _ as *const u8,
        util::size_of::<R>(),
        msg_off,
    )
}

pub fn drop_msgs(rep: EpId, label: Label) {
    TCU::drop_msgs_with(RBUFS[rep], rep, label);
}

pub fn fetch_msg(rep: EpId) -> Option<&'static Message> {
    TCU::fetch_msg(rep).map(|off| TCU::offset_to_msg(RBUFS[rep], off))
}

pub fn ack_msg(rep: EpId, msg: &Message) {
    let off = TCU::msg_to_offset(RBUFS[rep], msg);
    TCU::ack_msg(rep, off);
}

pub fn send_to(
    pe: PEId,
    ep: EpId,
    lbl: Label,
    msg: *const u8,
    size: usize,
    rpl_lbl: Label,
    rpl_ep: EpId,
) -> Result<(), Error> {
    config_local_ep(KTMP_EP, |regs| {
        let msg_ord = math::next_log2(size + util::size_of::<Header>());
        config_send(regs, KERNEL_ID, lbl, pe, ep, msg_ord, 1);
    });
    TCU::send(KTMP_EP, msg, size, rpl_lbl, rpl_ep)
}

pub fn read_obj<T>(pe: PEId, addr: goff) -> T {
    try_read_obj(pe, addr).unwrap()
}

pub fn try_read_obj<T>(pe: PEId, addr: goff) -> Result<T, Error> {
    let mut obj: T = unsafe { MaybeUninit::uninit().assume_init() };
    let obj_addr = &mut obj as *mut T as *mut u8;
    try_read_mem(pe, addr, obj_addr, util::size_of::<T>())?;
    Ok(obj)
}

pub fn read_slice<T>(pe: PEId, addr: goff, data: &mut [T]) {
    try_read_slice(pe, addr, data).unwrap();
}

pub fn try_read_slice<T>(pe: PEId, addr: goff, data: &mut [T]) -> Result<(), Error> {
    try_read_mem(
        pe,
        addr,
        data.as_mut_ptr() as *mut _ as *mut u8,
        data.len() * util::size_of::<T>(),
    )
}

pub fn read_mem(pe: PEId, addr: goff, data: *mut u8, size: usize) {
    try_read_mem(pe, addr, data, size).unwrap();
}

pub fn try_read_mem(pe: PEId, addr: goff, data: *mut u8, size: usize) -> Result<(), Error> {
    config_local_ep(KTMP_EP, |regs| {
        config_mem(regs, KERNEL_ID, pe, addr, size, kif::Perm::R);
    });
    TCU::read(KTMP_EP, data, size, 0)
}

pub fn write_slice<T>(pe: PEId, addr: goff, sl: &[T]) {
    let sl_addr = sl.as_ptr() as *const u8;
    write_mem(pe, addr, sl_addr, sl.len() * util::size_of::<T>());
}

pub fn try_write_slice<T>(pe: PEId, addr: goff, sl: &[T]) -> Result<(), Error> {
    let sl_addr = sl.as_ptr() as *const u8;
    try_write_mem(pe, addr, sl_addr, sl.len() * util::size_of::<T>())
}

pub fn write_mem(pe: PEId, addr: goff, data: *const u8, size: usize) {
    try_write_mem(pe, addr, data, size).unwrap();
}

pub fn try_write_mem(pe: PEId, addr: goff, data: *const u8, size: usize) -> Result<(), Error> {
    config_local_ep(KTMP_EP, |regs| {
        config_mem(regs, KERNEL_ID, pe, addr, size, kif::Perm::W);
    });
    TCU::write(KTMP_EP, data, size, 0)
}

pub fn clear(dst_pe: PEId, mut dst_addr: goff, size: usize) -> Result<(), Error> {
    let clear_size = cmp::min(size, BUF.len());
    unsafe {
        libc::memset(BUF.get_mut() as *mut _ as *mut libc::c_void, 0, clear_size);
    }

    let mut rem = size;
    while rem > 0 {
        let amount = cmp::min(rem, BUF.len());
        try_write_slice(dst_pe, dst_addr, &BUF[0..amount])?;
        dst_addr += amount as goff;
        rem -= amount;
    }
    Ok(())
}

pub fn copy(
    dst_pe: PEId,
    mut dst_addr: goff,
    src_pe: PEId,
    mut src_addr: goff,
    size: usize,
) -> Result<(), Error> {
    let mut rem = size;
    while rem > 0 {
        let amount = cmp::min(rem, BUF.len());
        try_read_slice(src_pe, src_addr, &mut BUF.get_mut()[0..amount])?;
        try_write_slice(dst_pe, dst_addr, &BUF[0..amount])?;
        src_addr += amount as goff;
        dst_addr += amount as goff;
        rem -= amount;
    }
    Ok(())
}
