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
use core::mem;
use core::ptr;
use errors::Error;
use goff;
use kif;
use libc;
use util;

mod backend;
mod thread;

pub type Reg = u64;
pub type EpId = usize;
pub type Label = u64;
pub type PEId = usize;

const PE_COUNT: usize = 16;
const MAX_MSG_SIZE: usize = 16 * 1024;

pub const HEADER_COUNT: usize = usize::max_value();

pub const EP_COUNT: EpId = 128;

pub const INVALID_EP: EpId = 0xFF;
pub const UNLIM_CREDITS: u32 = 0xFFFF_FFFF;

pub const SYSC_SEP_OFF: EpId = 0;
pub const SYSC_REP_OFF: EpId = 1;
pub const UPCALL_REP_OFF: EpId = 2;
pub const DEF_REP_OFF: EpId = 3;

pub const FIRST_USER_EP: EpId = 0;
pub const STD_EPS_COUNT: usize = 4;

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
        const PERM          = 15;
    }
}

// TODO temporary
bitflags! {
    /// The page table entry flags
    pub struct PTEFlags : u64 {
        /// Readable
        const R             = 0b0000_0001;
        /// Writable
        const W             = 0b0000_0010;
        /// Executable
        const X             = 0b0000_0100;
        /// Internally accessible, i.e., by the CU
        const I             = 0b0000_1000;
        /// Large page (2 MiB)
        const LARGE         = 0b0001_0000;
        /// Unsupported by TCU, but used for MMU
        const UNCACHED      = 0b0010_0000;
        /// Read+write
        const RW            = Self::R.bits | Self::W.bits;
        /// Read+write+execute
        const RWX           = Self::R.bits | Self::W.bits | Self::X.bits;
        /// Internal+read+write+execute
        const IRWX          = Self::R.bits | Self::W.bits | Self::X.bits | Self::I.bits;
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
        unsafe { intrinsics::transmute(Reg::from(cmd)) }
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

impl Message {
    /// Returns the message data as a reference to `T`.
    pub fn get_data<T>(&self) -> &T {
        assert!(mem::align_of_val(&self.data) >= mem::align_of::<T>());
        assert!(self.data.len() >= util::size_of::<T>());
        // safety: assuming that the size and alignment checks above works, the cast below is safe
        let slice = unsafe { &*(&self.data as *const [u8] as *const [T]) };
        &slice[0]
    }
}

pub const CMD_RCNT: usize = 8;
pub const EPS_RCNT: usize = 16;

static mut CMD_REGS: [Reg; CMD_RCNT] = [0; CMD_RCNT];

pub struct TCU {}

impl TCU {
    pub fn send(
        ep: EpId,
        msg: *const u8,
        size: usize,
        reply_lbl: Label,
        reply_ep: EpId,
    ) -> Result<(), Error> {
        Self::exec_command(ep, Command::SEND, msg, size, 0, 0, reply_lbl, reply_ep)
    }

    pub fn reply(ep: EpId, reply: *const u8, size: usize, msg_off: usize) -> Result<(), Error> {
        Self::exec_command(ep, Command::REPLY, reply, size, msg_off, 0, 0, 0)
    }

    pub fn read(
        ep: EpId,
        data: *mut u8,
        size: usize,
        off: goff,
        _flags: CmdFlags,
    ) -> Result<(), Error> {
        Self::exec_command(ep, Command::READ, data, size, off as usize, size, 0, 0)
    }

    pub fn write(
        ep: EpId,
        data: *const u8,
        size: usize,
        off: goff,
        _flags: CmdFlags,
    ) -> Result<(), Error> {
        Self::exec_command(ep, Command::WRITE, data, size, off as usize, size, 0, 0)
    }

    pub fn fetch_msg(ep: EpId) -> Option<usize> {
        if Self::get_ep(ep, EpReg::BUF_MSG_CNT) == 0 {
            return None;
        }

        Self::set_cmd(CmdReg::EPID, ep as Reg);
        Self::set_cmd(
            CmdReg::CTRL,
            (Command::FETCH_MSG.val << 3) | Control::START.bits,
        );
        if Self::get_command_result().is_err() {
            return None;
        }

        let msg = Self::get_cmd(CmdReg::OFFSET);
        if msg != !0 {
            Some(msg as usize)
        }
        else {
            None
        }
    }

    pub fn is_valid(ep: EpId) -> bool {
        Self::get_ep(ep, EpReg::VALID) == 1
    }

    pub fn ack_msg(ep: EpId, msg_off: usize) {
        Self::set_cmd(CmdReg::EPID, ep as Reg);
        Self::set_cmd(CmdReg::OFFSET, msg_off as Reg);
        Self::set_cmd(
            CmdReg::CTRL,
            (Command::ACK_MSG.val << 3) | Control::START.bits,
        );
        Self::get_command_result().unwrap();
    }

    pub fn sleep() -> Result<(), Error> {
        let time = libc::timespec {
            tv_nsec: 1000,
            tv_sec: 0,
        };
        unsafe {
            libc::nanosleep(&time, ptr::null_mut());
        }
        Ok(())
    }

    pub fn wait_for_msg(_ep: EpId) -> Result<(), Error> {
        Self::sleep()
    }

    pub fn configure(
        ep: EpId,
        lbl: Label,
        perm: kif::Perm,
        pe: PEId,
        dst_ep: EpId,
        crd: u64,
        msg_order: i32,
    ) {
        Self::set_ep(ep, EpReg::VALID, 1);
        Self::set_ep(ep, EpReg::LABEL, lbl);
        Self::set_ep(ep, EpReg::PE_ID, pe as Reg);
        Self::set_ep(ep, EpReg::EP_ID, dst_ep as Reg);
        Self::set_ep(ep, EpReg::CREDITS, crd);
        Self::set_ep(ep, EpReg::MSGORDER, msg_order as Reg);
        Self::set_ep(ep, EpReg::PERM, perm.bits() as Reg);
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

    pub fn drop_msgs_with(buf_addr: usize, ep: EpId, label: Label) {
        // we assume that the one that used the label can no longer send messages. thus, if there
        // are no messages yet, we are done.
        if Self::get_ep(ep, EpReg::BUF_MSG_CNT) == 0 {
            return;
        }

        let order = Self::get_ep(ep, EpReg::BUF_ORDER);
        let msg_order = Self::get_ep(ep, EpReg::BUF_MSGORDER);
        let unread = Self::get_ep(ep, EpReg::BUF_UNREAD);
        let max = 1 << (order - msg_order);
        for i in 0..max {
            if (unread & (1 << i)) != 0 {
                let msg = Self::offset_to_msg(buf_addr, i << msg_order);
                if msg.header.label == label {
                    Self::ack_msg(ep, (i << msg_order) as usize);
                }
            }
        }
    }

    pub fn offset_to_msg(base: usize, off: usize) -> &'static Message {
        unsafe {
            let msg_addr = arch::envdata::rbuf_start() + base + off;
            let head = msg_addr as *const Header;
            let slice = [msg_addr, (*head).length as usize];
            intrinsics::transmute(slice)
        }
    }

    pub fn msg_to_offset(base: usize, msg: &Message) -> usize {
        let addr = msg as *const _ as *const u8 as usize;
        addr - (arch::envdata::rbuf_start() + base)
    }

    #[allow(clippy::too_many_arguments)]
    fn exec_command(
        ep: EpId,
        cmd: Command,
        msg: *const u8,
        size: usize,
        off: usize,
        len: usize,
        reply_lbl: Label,
        reply_ep: EpId,
    ) -> Result<(), Error> {
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
            Self::set_cmd(
                CmdReg::CTRL,
                (cmd.val << 3) | (Control::START | Control::REPLY_CAP).bits,
            );
        }
        Self::get_command_result()
    }

    fn get_command_result() -> Result<(), Error> {
        while !Self::is_ready() {
            Self::sleep().unwrap();
        }

        Self::get_result()
    }

    fn is_ready() -> bool {
        (Self::get_cmd(CmdReg::CTRL) >> 3).trailing_zeros() >= 13
    }

    fn get_result() -> Result<(), Error> {
        match Self::get_cmd(CmdReg::CTRL) >> 16 {
            0 => Ok(()),
            e => Err(Error::from(e as u32)),
        }
    }

    fn get_cmd(cmd: CmdReg) -> Reg {
        unsafe { ptr::read_volatile(&CMD_REGS[cmd.val as usize]) }
    }

    fn set_cmd(cmd: CmdReg, val: Reg) {
        unsafe { ptr::write_volatile(&mut CMD_REGS[cmd.val as usize], val) }
    }

    fn get_ep(ep: EpId, reg: EpReg) -> Reg {
        unsafe { ptr::read_volatile(Self::ep_addr(ep, reg.val as usize)) }
    }

    fn set_ep(ep: EpId, reg: EpReg, val: Reg) {
        unsafe { ptr::write_volatile(Self::ep_addr(ep, reg.val as usize), val) }
    }

    fn ep_addr(ep: EpId, reg: usize) -> &'static mut Reg {
        let off = (ep * EPS_RCNT + reg as usize) * util::size_of::<Reg>();
        unsafe { intrinsics::transmute(arch::envdata::eps_start() + off) }
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
