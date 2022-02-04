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

use base::cell::{StaticCell, StaticRefCell};
use base::errors::{Code, Error};
use base::goff;
use base::kif;
use base::mem;
use base::tcu::{
    EpId, Header, Label, Message, Reg, TileId, AVAIL_EPS, EP_REGS, PMEM_PROT_EPS, TCU,
    UNLIM_CREDITS,
};

use crate::tiles::KERNEL_ID;

pub use crate::arch::ktcu::*;

pub const KSYS_EP: EpId = PMEM_PROT_EPS as EpId + 0;
pub const KSRV_EP: EpId = PMEM_PROT_EPS as EpId + 1;
pub const KTMP_EP: EpId = PMEM_PROT_EPS as EpId + 2;

#[cfg(not(target_vendor = "host"))]
static BUF: StaticRefCell<[u8; 8192]> = StaticRefCell::new([0u8; 8192]);
static RBUFS: StaticRefCell<[usize; 8]> = StaticRefCell::new([0usize; 8]);

pub fn config_local_ep<CFG>(ep: EpId, cfg: CFG)
where
    CFG: FnOnce(&mut [Reg]),
{
    let mut regs = [0; EP_REGS];
    cfg(&mut regs);
    TCU::set_ep_regs(ep, &regs);
}

pub fn config_remote_ep<CFG>(tile: TileId, ep: EpId, cfg: CFG) -> Result<(), Error>
where
    CFG: FnOnce(&mut [Reg]),
{
    let mut regs = [0; EP_REGS];
    cfg(&mut regs);
    write_ep_remote(tile, ep, &regs)
}

pub fn recv_msgs(ep: EpId, buf: goff, ord: u32, msg_ord: u32) -> Result<(), Error> {
    static REPS: StaticCell<EpId> = StaticCell::new(8);

    if REPS.get() + (1 << (ord - msg_ord)) > AVAIL_EPS {
        return Err(Error::new(Code::NoSpace));
    }

    let (buf, phys) = rbuf_addrs(buf);
    config_local_ep(ep, |regs| {
        config_recv(regs, KERNEL_ID, phys, ord, msg_ord, Some(REPS.get()));
        REPS.set(REPS.get() + (1 << (ord - msg_ord)));
    });
    RBUFS.borrow_mut()[ep as usize] = buf as usize;
    Ok(())
}

pub fn drop_msgs(rep: EpId, label: Label) {
    TCU::drop_msgs_with(RBUFS.borrow()[rep as usize], rep, label);
}

pub fn fetch_msg(rep: EpId) -> Option<&'static Message> {
    TCU::fetch_msg(rep).map(|off| TCU::offset_to_msg(RBUFS.borrow()[rep as usize], off))
}

pub fn ack_msg(rep: EpId, msg: &Message) {
    let off = TCU::msg_to_offset(RBUFS.borrow()[rep as usize], msg);
    TCU::ack_msg(rep, off).unwrap();
}

pub fn send_to(
    tile: TileId,
    ep: EpId,
    lbl: Label,
    msg: &mem::MsgBuf,
    rpl_lbl: Label,
    rpl_ep: EpId,
) -> Result<(), Error> {
    config_local_ep(KTMP_EP, |regs| {
        // don't calculate the msg order here, because it can take some time and it doesn't really
        // matter what we set here assuming that it's large enough.
        assert!(msg.size() + mem::size_of::<Header>() <= 1 << 8);
        config_send(regs, KERNEL_ID, lbl, tile, ep, 8, UNLIM_CREDITS);
    });
    klog!(
        KTCU,
        "sending {}-bytes from {:#x} to {}:{}",
        msg.size(),
        msg.bytes().as_ptr() as usize,
        tile,
        ep
    );
    TCU::send(KTMP_EP, msg, rpl_lbl, rpl_ep)
}

pub fn reply(ep: EpId, reply: &mem::MsgBuf, msg: &Message) -> Result<(), Error> {
    let msg_off = TCU::msg_to_offset(RBUFS.borrow()[ep as usize], msg);
    TCU::reply(ep, reply, msg_off)
}

#[cfg(not(target_vendor = "host"))]
pub fn read_obj<T>(tile: TileId, addr: goff) -> T {
    try_read_obj(tile, addr).unwrap()
}

#[cfg(not(target_vendor = "host"))]
pub fn try_read_obj<T>(tile: TileId, addr: goff) -> Result<T, Error> {
    use base::mem::MaybeUninit;

    #[allow(clippy::uninit_assumed_init)]
    let mut obj: T = unsafe { MaybeUninit::uninit().assume_init() };
    let obj_addr = &mut obj as *mut T as *mut u8;
    try_read_mem(tile, addr, obj_addr, mem::size_of::<T>())?;
    Ok(obj)
}

#[cfg(not(target_vendor = "host"))]
pub fn read_slice<T>(tile: TileId, addr: goff, data: &mut [T]) {
    try_read_slice(tile, addr, data).unwrap();
}

#[cfg(not(target_vendor = "host"))]
pub fn try_read_slice<T>(tile: TileId, addr: goff, data: &mut [T]) -> Result<(), Error> {
    try_read_mem(
        tile,
        addr,
        data.as_mut_ptr() as *mut _ as *mut u8,
        data.len() * mem::size_of::<T>(),
    )
}

#[cfg(not(target_vendor = "host"))]
pub fn try_read_mem(tile: TileId, addr: goff, data: *mut u8, size: usize) -> Result<(), Error> {
    config_local_ep(KTMP_EP, |regs| {
        config_mem(regs, KERNEL_ID, tile, addr, size, kif::Perm::R);
    });
    klog!(KTCU, "reading {} bytes from {}:{:#x}", size, tile, addr);
    TCU::read(KTMP_EP, data, size, 0)
}

#[cfg(not(target_vendor = "host"))]
pub fn write_slice<T>(tile: TileId, addr: goff, sl: &[T]) {
    let sl_addr = sl.as_ptr() as *const u8;
    write_mem(tile, addr, sl_addr, sl.len() * mem::size_of::<T>());
}

#[cfg(not(target_vendor = "host"))]
pub fn try_write_slice<T>(tile: TileId, addr: goff, sl: &[T]) -> Result<(), Error> {
    let sl_addr = sl.as_ptr() as *const u8;
    try_write_mem(tile, addr, sl_addr, sl.len() * mem::size_of::<T>())
}

#[cfg(not(target_vendor = "host"))]
pub fn write_mem(tile: TileId, addr: goff, data: *const u8, size: usize) {
    try_write_mem(tile, addr, data, size).unwrap();
}

pub fn try_write_mem(tile: TileId, addr: goff, data: *const u8, size: usize) -> Result<(), Error> {
    config_local_ep(KTMP_EP, |regs| {
        config_mem(regs, KERNEL_ID, tile, addr, size, kif::Perm::W);
    });
    klog!(KTCU, "writing {} bytes to {}:{:#x}", size, tile, addr);
    TCU::write(KTMP_EP, data, size, 0)
}

#[cfg(not(target_vendor = "host"))]
pub fn clear(dst_tile: TileId, mut dst_addr: goff, size: usize) -> Result<(), Error> {
    use base::libc;

    let mut buf = BUF.borrow_mut();
    let clear_size = core::cmp::min(size, buf.len());
    unsafe {
        libc::memset(buf.as_mut_ptr() as *mut libc::c_void, 0, clear_size);
    }

    let mut rem = size;
    while rem > 0 {
        let amount = core::cmp::min(rem, buf.len());
        try_write_slice(dst_tile, dst_addr, &buf[0..amount])?;
        dst_addr += amount as goff;
        rem -= amount;
    }
    Ok(())
}

#[cfg(not(target_vendor = "host"))]
pub fn copy(
    dst_tile: TileId,
    mut dst_addr: goff,
    src_tile: TileId,
    mut src_addr: goff,
    size: usize,
) -> Result<(), Error> {
    let mut buf = BUF.borrow_mut();
    let mut rem = size;
    while rem > 0 {
        let amount = core::cmp::min(rem, buf.len());
        try_read_slice(src_tile, src_addr, &mut buf[0..amount])?;
        try_write_slice(dst_tile, dst_addr, &buf[0..amount])?;
        src_addr += amount as goff;
        dst_addr += amount as goff;
        rem -= amount;
    }
    Ok(())
}
