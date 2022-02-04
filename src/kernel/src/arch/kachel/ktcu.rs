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

use base::cfg;
use base::errors::{Code, Error};
use base::goff;
use base::kif::{PageFlags, Perm};
use base::mem::GlobAddr;
use base::tcu::*;

use crate::arch;
use crate::ktcu;
use crate::platform;

pub const KPEX_EP: EpId = PMEM_PROT_EPS as EpId + 3;

pub fn rbuf_addrs(virt: goff) -> (goff, goff) {
    if platform::tile_desc(platform::kernel_tile()).has_virtmem() {
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

pub fn deprivilege_tile(tile: TileId) -> Result<(), Error> {
    let mut features: u64 = ktcu::try_read_obj(tile, TCU::ext_reg_addr(ExtReg::FEATURES) as goff)?;
    features &= !1;
    ktcu::try_write_slice(tile, TCU::ext_reg_addr(ExtReg::FEATURES) as goff, &[
        features,
    ])
}

pub fn reset_tile(tile: TileId, _pid: i32) -> Result<(), Error> {
    let value = ExtCmdOpCode::RESET.val as Reg;
    do_ext_cmd(tile, value).map(|_| ())
}

pub fn config_recv(
    regs: &mut [Reg],
    act: ActId,
    buf: goff,
    buf_ord: u32,
    msg_ord: u32,
    reply_eps: Option<EpId>,
) {
    TCU::config_recv(regs, act, buf, buf_ord, msg_ord, reply_eps);
}

pub fn config_send(
    regs: &mut [Reg],
    act: ActId,
    lbl: Label,
    tile: TileId,
    dst_ep: EpId,
    msg_order: u32,
    credits: u32,
) {
    TCU::config_send(regs, act, lbl, tile, dst_ep, msg_order, credits);
}

pub fn config_mem(regs: &mut [Reg], act: ActId, tile: TileId, addr: goff, size: usize, perm: Perm) {
    TCU::config_mem(regs, act, tile, addr, size, perm);
}

pub fn glob_to_phys_remote(tile: TileId, glob: GlobAddr, flags: PageFlags) -> Result<goff, Error> {
    glob.to_phys_with(flags, |ep| {
        let mut regs = [0; 3];
        if ktcu::read_ep_remote(tile, ep, &mut regs).is_ok() {
            TCU::unpack_mem_regs(&regs)
        }
        else {
            None
        }
    })
}

pub fn read_ep_remote(tile: TileId, ep: EpId, regs: &mut [Reg]) -> Result<(), Error> {
    for i in 0..regs.len() {
        ktcu::try_read_slice(
            tile,
            (TCU::ep_regs_addr(ep) + i * 8) as goff,
            &mut regs[i..i + 1],
        )?;
    }
    Ok(())
}

pub fn write_ep_remote(tile: TileId, ep: EpId, regs: &[Reg]) -> Result<(), Error> {
    for (i, r) in regs.iter().enumerate() {
        ktcu::try_write_slice(tile, (TCU::ep_regs_addr(ep) + i * 8) as goff, &[*r])?;
    }
    Ok(())
}

pub fn invalidate_ep_remote(tile: TileId, ep: EpId, force: bool) -> Result<u32, Error> {
    let reg = ExtCmdOpCode::INV_EP.val | ((ep as Reg) << 9) as Reg | ((force as Reg) << 25);
    do_ext_cmd(tile, reg).map(|unread| unread as u32)
}

pub fn inv_reply_remote(
    recv_tile: TileId,
    recv_ep: EpId,
    send_tile: TileId,
    send_ep: EpId,
) -> Result<(), Error> {
    let mut regs = [0; EP_REGS];
    read_ep_remote(recv_tile, recv_ep, &mut regs)?;

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
            read_ep_remote(recv_tile, reply_eps + i, &mut regs)?;

            // is that replying to the sender?
            let tgt_tile = ((regs[1] >> 16) & 0xFFFF) as TileId;
            let crd_ep = ((regs[0] >> 37) & 0xFFFF) as EpId;
            if crd_ep == send_ep && tgt_tile == send_tile {
                ktcu::invalidate_ep_remote(recv_tile, reply_eps + i, true)?;
            }
        }
    }

    Ok(())
}

fn do_ext_cmd(tile: TileId, cmd: Reg) -> Result<Reg, Error> {
    let addr = TCU::ext_reg_addr(ExtReg::EXT_CMD) as goff;
    ktcu::try_write_slice(tile, addr, &[cmd])?;

    let res = loop {
        let res: Reg = ktcu::try_read_obj(tile, addr)?;
        if (res & 0xF) == ExtCmdOpCode::IDLE.val {
            break res;
        }
    };

    match Code::from(((res >> 4) & 0x1F) as u32) {
        Code::None => Ok(res >> 9),
        e => Err(Error::new(e)),
    }
}
