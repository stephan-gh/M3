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
use base::errors::Error;
use base::goff;
use base::kif::{PageFlags, Perm};
use base::libc;
use base::tcu::*;
use base::util;
use core::cmp;
use core::mem::MaybeUninit;

use arch;
use ktcu;
use pes::{VPEId, KERNEL_ID};
use platform;

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
    do_ext_cmd(pe, value)
}

pub fn config_recv(
    regs: &mut [Reg],
    vpe: VPEId,
    buf: goff,
    buf_ord: u32,
    msg_ord: u32,
    reply_eps: Option<EpId>,
) {
    regs[0] = EpType::RECEIVE.val
        | ((vpe as Reg) << 3)
        | ((reply_eps.unwrap_or(NO_REPLIES) as Reg) << 19)
        | (((buf_ord - msg_ord) as Reg) << 35)
        | ((msg_ord as Reg) << 41);
    regs[1] = buf as Reg;
    regs[2] = 0;
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
    regs[0] = EpType::SEND.val
        | ((vpe as Reg) << 3)
        | ((credits as Reg) << 19)
        | ((credits as Reg) << 25)
        | ((msg_order as Reg) << 31);
    regs[1] = ((pe as Reg) << 16) | (dst_ep as Reg);
    regs[2] = lbl as Reg;
}

pub fn config_mem(regs: &mut [Reg], vpe: VPEId, pe: PEId, addr: goff, size: usize, perm: Perm) {
    regs[0] = EpType::MEMORY.val
        | ((vpe as Reg) << 3)
        | ((perm.bits() as Reg) << 19)
        | ((pe as Reg) << 23);
    regs[1] = addr as Reg;
    regs[2] = size as Reg;
}

pub fn write_ep_remote(pe: PEId, ep: EpId, regs: &[Reg]) -> Result<(), Error> {
    ktcu::try_write_slice(pe, TCU::ep_regs_addr(ep) as goff, &regs)
}

pub fn invalidate_ep_remote(pe: PEId, ep: EpId, force: bool) -> Result<(), Error> {
    let reg = ExtCmdOpCode::INV_EP.val | (ep << 8) as Reg | ((force as Reg) << 24);
    do_ext_cmd(pe, reg)
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

fn do_ext_cmd(pe: PEId, cmd: Reg) -> Result<(), Error> {
    ktcu::try_write_slice(pe, TCU::ext_reg_addr(ExtReg::EXT_CMD) as goff, &[cmd])
}
