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
use base::cfg;
use base::errors::{Code, Error};
use base::goff;
use base::kif::{PageFlags, Perm};
use base::libc;
use base::tcu::*;
use base::util;
use core::cmp;
use core::mem::MaybeUninit;

use crate::arch;
use crate::ktcu;
use crate::pes::KERNEL_ID;
use crate::platform;

pub const KPEX_EP: EpId = 3;

static BUF: StaticCell<[u8; 8192]> = StaticCell::new([0u8; 8192]);

pub fn rbuf_addrs(virt: goff) -> (goff, goff) {
    if platform::pe_desc(platform::kernel_pe()).has_virtmem() {
        let pte = arch::paging::translate(virt as usize, PageFlags::R);
        (
            virt,
            (pte & !(cfg::PAGE_MASK as goff)) | (virt & cfg::PAGE_MASK as goff),
        )
    }
    else {
        (virt, virt)
    }
}

pub fn deprivilege_pe(pe: PEId) -> Result<(), Error> {
    let features = 0;
    ktcu::try_write_mem(
        pe,
        TCU::ext_reg_addr(ExtReg::FEATURES) as goff,
        &features,
        util::size_of::<Reg>(),
    )
}

pub fn reset_pe(pe: PEId, _pid: i32) -> Result<(), Error> {
    let value = ExtCmdOpCode::RESET.val as Reg;
    do_ext_cmd(pe, value).map(|_| ())
}

pub fn config_recv(
    regs: &mut [Reg],
    vpe: VPEId,
    buf: goff,
    buf_ord: u32,
    msg_ord: u32,
    reply_eps: Option<EpId>,
) {
    TCU::config_recv(regs, vpe, buf, buf_ord, msg_ord, reply_eps);
}

pub fn config_send(
    regs: &mut [Reg],
    vpe: VPEId,
    lbl: Label,
    pe: PEId,
    dst_ep: EpId,
    msg_order: u32,
    credits: u32,
) {
    TCU::config_send(regs, vpe, lbl, pe, dst_ep, msg_order, credits);
}

pub fn config_mem(regs: &mut [Reg], vpe: VPEId, pe: PEId, addr: goff, size: usize, perm: Perm) {
    TCU::config_mem(regs, vpe, pe, addr, size, perm);
}

pub fn write_ep_remote(pe: PEId, ep: EpId, regs: &[Reg]) -> Result<(), Error> {
    ktcu::try_write_slice(pe, TCU::ep_regs_addr(ep) as goff, &regs)
}

pub fn invalidate_ep_remote(pe: PEId, ep: EpId, force: bool) -> Result<u32, Error> {
    let reg = ExtCmdOpCode::INV_EP.val | ((ep as Reg) << 9) as Reg | ((force as Reg) << 25);
    do_ext_cmd(pe, reg).map(|unread| unread as u32)
}

pub fn inv_reply_remote(
    recv_pe: PEId,
    recv_ep: EpId,
    send_pe: PEId,
    send_ep: EpId,
) -> Result<(), Error> {
    let mut regs = [0 as Reg; EP_REGS];
    ktcu::try_read_slice(recv_pe, TCU::ep_regs_addr(recv_ep) as goff, &mut regs)?;

    // if there is no occupied slot, there can't be any reply EP we have to invalidate
    let occupied = regs[2] & 0xFFFF_FFFF;
    if occupied == 0 {
        return Ok(());
    }

    let buf_size = 1 << ((regs[0] >> 35) & 0x3F);
    let reply_eps = ((regs[0] >> 19) & 0xFFFF) as EpId;
    for i in 0..buf_size {
        if (occupied & (1 << i)) != 0 {
            // load the reply EP
            ktcu::try_read_slice(recv_pe, TCU::ep_regs_addr(reply_eps + i) as goff, &mut regs)?;

            // is that replying to the sender?
            let tgt_pe = ((regs[1] >> 16) & 0xFFFF) as PEId;
            let crd_ep = ((regs[0] >> 37) & 0xFFFF) as EpId;
            if crd_ep == send_ep && tgt_pe == send_pe {
                ktcu::invalidate_ep_remote(recv_pe, reply_eps + i, true)?;
            }
        }
    }

    Ok(())
}

pub fn read_obj<T>(pe: PEId, addr: goff) -> T {
    try_read_obj(pe, addr).unwrap()
}

pub fn try_read_obj<T>(pe: PEId, addr: goff) -> Result<T, Error> {
    #[allow(clippy::uninit_assumed_init)]
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

pub fn try_read_mem(pe: PEId, addr: goff, data: *mut u8, size: usize) -> Result<(), Error> {
    ktcu::config_local_ep(ktcu::KTMP_EP, |regs| {
        config_mem(regs, KERNEL_ID, pe, addr, size, Perm::R);
    });
    TCU::read(ktcu::KTMP_EP, data, size, 0)
}

pub fn write_slice<T>(pe: PEId, addr: goff, sl: &[T]) {
    let sl_addr = sl.as_ptr() as *const u8;
    write_mem(pe, addr, sl_addr, sl.len() * util::size_of::<T>());
}

pub fn try_write_slice<T>(pe: PEId, addr: goff, sl: &[T]) -> Result<(), Error> {
    let sl_addr = sl.as_ptr() as *const u8;
    ktcu::try_write_mem(pe, addr, sl_addr, sl.len() * util::size_of::<T>())
}

pub fn write_mem(pe: PEId, addr: goff, data: *const u8, size: usize) {
    ktcu::try_write_mem(pe, addr, data, size).unwrap();
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

fn do_ext_cmd(pe: PEId, cmd: Reg) -> Result<Reg, Error> {
    let addr = TCU::ext_reg_addr(ExtReg::EXT_CMD) as goff;
    ktcu::try_write_slice(pe, addr, &[cmd])?;

    let res = loop {
        let res: Reg = ktcu::try_read_obj(pe, addr)?;
        if (res & 0xF) == ExtCmdOpCode::IDLE.val {
            break res;
        }
    };

    match Code::from(((res >> 4) & 0x1F) as u32) {
        Code::None => Ok(res >> 9),
        e => Err(Error::new(e)),
    }
}
