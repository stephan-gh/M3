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
use base::tcu::{EpId, Header, Label, Message, PEId, Reg, EP_COUNT, EP_REGS, TCU, UNLIM_CREDITS};
use base::util;

use crate::pes::KERNEL_ID;

pub use crate::arch::ktcu::*;

pub const KSYS_EP: EpId = 0;
pub const KSRV_EP: EpId = 1;
pub const KTMP_EP: EpId = 2;

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
    static REPS: StaticCell<EpId> = StaticCell::new(8);

    if *REPS + (1 << (ord - msg_ord)) > EP_COUNT {
        return Err(Error::new(Code::NoSpace));
    }

    let (buf, phys) = rbuf_addrs(buf);
    config_local_ep(ep, |regs| {
        config_recv(regs, KERNEL_ID, phys, ord, msg_ord, Some(*REPS));
        *REPS.get_mut() += 1 << (ord - msg_ord);
    });
    RBUFS.get_mut()[ep as usize] = buf as usize;
    Ok(())
}

pub fn reply<R>(ep: EpId, reply: &R, msg: &Message) -> Result<(), Error> {
    let msg_off = TCU::msg_to_offset(RBUFS[ep as usize], msg);
    TCU::reply(
        ep,
        reply as *const _ as *const u8,
        util::size_of::<R>(),
        msg_off,
    )
}

pub fn drop_msgs(rep: EpId, label: Label) {
    TCU::drop_msgs_with(RBUFS[rep as usize], rep, label);
}

pub fn fetch_msg(rep: EpId) -> Option<&'static Message> {
    TCU::fetch_msg(rep).map(|off| TCU::offset_to_msg(RBUFS[rep as usize], off))
}

pub fn ack_msg(rep: EpId, msg: &Message) {
    let off = TCU::msg_to_offset(RBUFS[rep as usize], msg);
    TCU::ack_msg(rep, off).unwrap();
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
        // don't calculate the msg order here, because it can take some time and it doesn't really
        // matter what we set here assuming that it's large enough.
        assert!(size + util::size_of::<Header>() <= 1 << 8);
        config_send(regs, KERNEL_ID, lbl, pe, ep, 8, UNLIM_CREDITS);
    });
    klog!(KTCU, "sending {}-bytes from {:#x} to {}:{}", size, msg as usize, pe, ep);
    TCU::send(KTMP_EP, msg, size, rpl_lbl, rpl_ep)
}

pub fn try_write_mem(pe: PEId, addr: goff, data: *const u8, size: usize) -> Result<(), Error> {
    config_local_ep(KTMP_EP, |regs| {
        config_mem(regs, KERNEL_ID, pe, addr, size, kif::Perm::W);
    });
    klog!(KTCU, "writing {} bytes to {}:{:#x}", size, pe, addr);
    TCU::write(KTMP_EP, data, size, 0)
}
