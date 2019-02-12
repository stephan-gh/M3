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

use arch;
use cfg;
use const_assert;
use core::intrinsics;
use core::ptr;
use errors::Error;
use goff;
use libc;
use util;

mod backend;
mod thread;

pub type Reg    = u64;
pub type EpId   = usize;
pub type Label  = u64;
pub type PEId   = usize;

const PE_COUNT: usize           = 16;
const MAX_MSG_SIZE: usize       = 16 * 1024;

pub const HEADER_COUNT: usize   = usize::max_value();

pub const EP_COUNT: EpId        = 16;

pub const SYSC_SEP: EpId        = 0;
pub const SYSC_REP: EpId        = 1;
pub const UPCALL_REP: EpId      = 2;
pub const DEF_REP: EpId         = 3;
pub const FIRST_FREE_EP: EpId   = 4;

int_enum! {
    struct CmdReg : Reg {
        const ADDR          = 0;
        const SIZE          = 1;
        const EPID          = 2;
        const CTRL          = 3;
        const OFFSET        = 4;
        const REPLY_LBL     = 5;
        const REPLY_EPID    = 6;
        const LENGTH        = 7;
    }
}

int_enum! {
    pub struct EpReg : Reg {
        const VALID         = 0;

        // receive buffer registers
        const BUF_ADDR      = 1;
        const BUF_ORDER     = 2;
        const BUF_MSGORDER  = 3;
        const BUF_ROFF      = 4;
        const BUF_WOFF      = 5;
        const BUF_MSG_CNT   = 6;
        const BUF_MSG_ID    = 7;
        const BUF_UNREAD    = 8;
        const BUF_OCCUPIED  = 9;

        // for sending message and accessing memory
        const PE_ID         = 10;
        const EP_ID         = 11;
        const LABEL         = 12;
        const CREDITS       = 13;
        const MSGORDER      = 14;
    }
}

int_enum! {
    struct Command : Reg {
        const READ          = 1;
        const WRITE         = 2;
        const SEND          = 3;
        const REPLY         = 4;
        const RESP          = 5;
        const FETCH_MSG     = 6;
        const ACK_MSG       = 7;
    }
}

impl From<u8> for Command {
    fn from(cmd: u8) -> Self {
        unsafe { intrinsics::transmute(cmd as Reg) }
    }
}

bitflags! {
    struct Control : Reg {
        const NONE        = 0b000;
        const START       = 0b001;
        const REPLY_CAP   = 0b010;
    }
}

bitflags! {
    pub struct CmdFlags : u64 {
        const NOPF        = 0x1;
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone, Debug)]
pub struct Header {
    pub length: usize,
    pub opcode: u8,
    pub label: Label,
    pub has_replycap: u8,
    pub pe: u16,
    pub rpl_ep: u8,
    pub snd_ep: u8,
    pub reply_label: Label,
    pub credits: u8,
    pub crd_ep: u8,
}

impl Header {
    const fn new() -> Header {
        Header {
            length: 0,
            opcode: 0,
            label: 0,
            has_replycap: 0,
            pe: 0,
            rpl_ep: 0,
            snd_ep: 0,
            reply_label: 0,
            credits: 0,
            crd_ep: 0,
        }
    }
}

#[repr(C, packed)]
#[derive(Debug)]
pub struct Message {
    pub header: Header,
    pub data: [u8],
}

pub const CMD_RCNT: usize = 8;
pub const EPS_RCNT: usize = 15;

static mut CMD_REGS: [Reg; CMD_RCNT] = [0; CMD_RCNT];

pub struct DTU {
}

impl DTU {
    pub fn send(ep: EpId, msg: *const u8, size: usize, reply_lbl: Label, reply_ep: EpId) -> Result<(), Error> {
        Self::exec_command(ep, Command::SEND, msg, size, 0, 0, reply_lbl, reply_ep)
    }

    pub fn reply(ep: EpId, reply: *const u8, size: usize, msg: &'static Message) -> Result<(), Error> {
        let msg_addr = msg as *const Message as *const u8 as usize;
        Self::exec_command(ep, Command::REPLY, reply, size, msg_addr, 0, 0, 0)
    }

    pub fn read(ep: EpId, data: *mut u8, size: usize, off: goff, _flags: CmdFlags) -> Result<(), Error> {
        Self::exec_command(ep, Command::READ, data, size, off as usize, size, 0, 0)
    }

    pub fn write(ep: EpId, data: *const u8, size: usize, off: goff, _flags: CmdFlags) -> Result<(), Error> {
        Self::exec_command(ep, Command::WRITE, data, size, off as usize, size, 0, 0)
    }

    pub fn fetch_msg(ep: EpId) -> Option<&'static Message> {
        if Self::get_ep(ep, EpReg::BUF_MSG_CNT) == 0 {
            return None;
        }

        Self::set_cmd(CmdReg::EPID, ep as Reg);
        Self::set_cmd(CmdReg::CTRL, (Command::FETCH_MSG.val << 3) | Control::START.bits);
        if Self::get_command_result().is_err() {
            return None;
        }

        let msg = Self::get_cmd(CmdReg::OFFSET);
        if msg != 0 {
            unsafe {
                let head: *const Header = intrinsics::transmute(msg);
                let slice: [usize; 2] = [msg as usize, (*head).length as usize];
                Some(intrinsics::transmute(slice))
            }
        }
        else {
            None
        }
    }

    pub fn fetch_events() -> Reg {
        0
    }

    pub fn is_valid(ep: EpId) -> bool {
        Self::get_ep(ep, EpReg::VALID) == 1
    }

    pub fn mark_read(ep: EpId, msg: &Message) {
        let msg_addr = msg as *const Message as *const u8 as usize;
        Self::set_cmd(CmdReg::EPID, ep as Reg);
        Self::set_cmd(CmdReg::OFFSET, msg_addr as Reg);
        Self::set_cmd(CmdReg::CTRL, (Command::ACK_MSG.val << 3) | Control::START.bits);
        Self::get_command_result().unwrap();
    }

    pub fn try_sleep(_yield: bool, _cycles: u64) -> Result<(), Error> {
        unsafe { libc::usleep(1) };
        Ok(())
    }

    pub fn configure(ep: EpId, lbl: Label, pe: PEId, dst_ep: EpId, crd: u64, msg_order: i32) {
        Self::set_ep(ep, EpReg::VALID, 1);
        Self::set_ep(ep, EpReg::LABEL, lbl);
        Self::set_ep(ep, EpReg::PE_ID, pe as Reg);
        Self::set_ep(ep, EpReg::EP_ID, dst_ep as Reg);
        Self::set_ep(ep, EpReg::CREDITS, crd);
        Self::set_ep(ep, EpReg::MSGORDER, msg_order as Reg);
    }
    pub fn configure_recv(ep: EpId, buf: usize, order: i32, msg_order: i32) {
        Self::set_ep(ep, EpReg::VALID, 1);
        Self::set_ep(ep, EpReg::BUF_ADDR, buf as Reg);
        Self::set_ep(ep, EpReg::BUF_ORDER, order as Reg);
        Self::set_ep(ep, EpReg::BUF_MSGORDER, msg_order as Reg);
        Self::set_ep(ep, EpReg::BUF_ROFF, 0);
        Self::set_ep(ep, EpReg::BUF_WOFF, 0);
        Self::set_ep(ep, EpReg::BUF_MSG_CNT, 0);
        Self::set_ep(ep, EpReg::BUF_UNREAD, 0);
        Self::set_ep(ep, EpReg::BUF_OCCUPIED, 0);
    }

    fn exec_command(ep: EpId, cmd: Command, msg: *const u8, size: usize, off: usize, len: usize,
            reply_lbl: Label, reply_ep: EpId) -> Result<(), Error> {
        Self::set_cmd(CmdReg::ADDR, msg as Reg);
        Self::set_cmd(CmdReg::SIZE, size as Reg);
        Self::set_cmd(CmdReg::EPID, ep as Reg);
        Self::set_cmd(CmdReg::OFFSET, off as Reg);
        Self::set_cmd(CmdReg::LENGTH, len as Reg);
        Self::set_cmd(CmdReg::REPLY_LBL, reply_lbl as Reg);
        Self::set_cmd(CmdReg::REPLY_EPID, reply_ep as Reg);
        if cmd == Command::REPLY {
            Self::set_cmd(CmdReg::CTRL, (cmd.val << 3) | Control::START.bits);
        }
        else {
            Self::set_cmd(CmdReg::CTRL, (cmd.val << 3) | (Control::START | Control::REPLY_CAP).bits);
        }
        Self::get_command_result()
    }

    fn get_command_result() -> Result<(), Error> {
        while !Self::is_ready() {
            Self::try_sleep(false, 0).unwrap();
        }

        Self::get_result()
    }

    fn is_ready() -> bool {
        (Self::get_cmd(CmdReg::CTRL) >> 3) & 0x1FFF == 0
    }
    fn get_result() -> Result<(), Error> {
        match Self::get_cmd(CmdReg::CTRL) >> 16 {
            0 => Ok(()),
            e => Err(Error::from(e as u32)),
        }
    }

    fn get_cmd(cmd: CmdReg) -> Reg {
        unsafe {
            ptr::read_volatile(&CMD_REGS[cmd.val as usize])
        }
    }
    fn set_cmd(cmd: CmdReg, val: Reg) {
        unsafe {
            ptr::write_volatile(&mut CMD_REGS[cmd.val as usize], val)
        }
    }

    fn get_ep(ep: EpId, reg: EpReg) -> Reg {
        unsafe {
            ptr::read_volatile(Self::ep_addr(ep, reg.val as usize))
        }
    }
    fn set_ep(ep: EpId, reg: EpReg, val: Reg) {
        unsafe {
            ptr::write_volatile(Self::ep_addr(ep, reg.val as usize), val)
        }
    }

    fn ep_addr(ep: EpId, reg: usize) -> &'static mut Reg {
        let off = (ep * EPS_RCNT + reg as usize) * util::size_of::<Reg>();
        unsafe {
            intrinsics::transmute(arch::envdata::eps_start() + off)
        }
    }
}

#[cfg(feature = "kernel")]
impl DTU {
    pub fn set_ep_regs(ep: EpId, regs: &[Reg]) {
        for i in 0..EPS_RCNT {
            unsafe {
                ptr::write_volatile(Self::ep_addr(ep, i), regs[i])
            }
        }
    }
}

pub fn init() {
    const EP_SIZE: usize = (EP_COUNT * EPS_RCNT) * util::size_of::<Reg>();
    const_assert!(EP_SIZE <= cfg::EPMEM_SIZE);

    thread::init();
}

pub fn deinit() {
    thread::deinit();
}
