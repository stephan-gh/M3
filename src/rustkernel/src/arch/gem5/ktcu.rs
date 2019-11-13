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

use base::errors::Error;
use base::goff;
use base::cfg;
use base::kif::{Perm, PageFlags};
use base::tcu::*;
use base::util;

use arch;
use ktcu;
use pes::VPEId;
use platform;

pub fn rbuf_addrs(virt: goff) -> (goff, goff) {
    if platform::pe_desc(platform::kernel_pe()).has_virtmem() {
        let pte = arch::paging::translate(virt as usize, PageFlags::R);
        (virt, (pte & !(cfg::PAGE_MASK as goff)) | (virt & cfg::PAGE_MASK as goff))
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

pub fn reset_pe(pe: PEId) -> Result<(), Error> {
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

fn do_ext_cmd(pe: PEId, cmd: Reg) -> Result<(), Error> {
    ktcu::try_write_slice(pe, TCU::ext_reg_addr(ExtReg::EXT_CMD) as goff, &[cmd])
}
