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
use core::intrinsics;
use errors::{Code, Error};
use goff;
use util;

/// A DTU register
pub type Reg = u64;
/// An endpoint id
pub type EpId = usize;
/// A DTU label used in send EPs
pub type Label = u64;
/// A PE id
pub type PEId = usize;

/// The number of endpoints in each DTU
pub const EP_COUNT: EpId = 16;

/// The send EP for kernel calls from PEMux
pub const KPEX_SEP: EpId = 0;
/// The receive EP for kernel calls from PEMux
pub const KPEX_REP: EpId = 1;
/// The send EP for system calls
pub const SYSC_SEP: EpId = 2;
/// The receive EP for system calls
pub const SYSC_REP: EpId = 3;
/// The receive EP for upcalls from the kernel
pub const UPCALL_REP: EpId = 4;
/// The default receive EP
pub const DEF_REP: EpId = 5;
/// The first free EP id
pub const FIRST_FREE_EP: EpId = 6;

/// The base address of the DTU's MMIO area
pub const BASE_ADDR: usize = 0xF000_0000;
/// The base address of the DTU's MMIO area for external requests
pub const BASE_REQ_ADDR: usize = BASE_ADDR + cfg::PAGE_SIZE;
/// The number of DTU registers
pub const DTU_REGS: usize = 8;
/// The number of command registers
pub const CMD_REGS: usize = 5;
/// The number of registers per EP
pub const EP_REGS: usize = 3;
/// The number of headers per DTU
pub const HEADER_COUNT: usize = 128;
/// The number of registers per header
pub const HEADER_REGS: usize = 2;

/// Represents unlimited credits
pub const CREDITS_UNLIM: u64 = 0xFFFF;

// actual max is 64k - 1; use less for better alignment
const MAX_PKT_SIZE: usize = 60 * 1024;

int_enum! {
    /// The DTU registers
    pub struct DtuReg : Reg {
        /// Stores various status flags
        const STATUS      = 0;
        const ROOT_PT     = 1;
        const PF_EP       = 2;
        const CUR_TIME    = 3;
        const EVENTS      = 4;
        const EXT_CMD     = 5;
        const CLEAR_IRQ   = 6;
        const CLOCK       = 7;
    }
}

#[allow(dead_code)]
bitflags! {
    /// The status flag for the `DtuReg::STATUS` register
    pub struct StatusFlags : Reg {
        /// Whether the PE is privileged
        const PRIV         = 1 << 0;
        /// Whether page faults are send via `PF_EP`
        const PAGEFAULTS   = 1 << 1;
    }
}

#[allow(dead_code)]
int_enum! {
    /// The request registers
    pub struct ReqReg : Reg {
        /// For external requests
        const EXT_REQ     = 0x0;
        /// For translation requests
        const XLATE_REQ   = 0x1;
        /// For translation responses
        const XLATE_RESP  = 0x2;
    }
}

#[allow(dead_code)]
int_enum! {
    /// The command registers
    pub struct CmdReg : Reg {
        /// Starts commands and signals their completion
        const COMMAND     = 0x0;
        /// Aborts commands
        const ABORT       = 0x1;
        /// Specifies the data address and size
        const DATA        = 0x2;
        /// Specifies an offset
        const OFFSET      = 0x3;
        /// Specifies the reply label
        const REPLY_LABEL = 0x4;
    }
}

int_enum! {
    /// The commands
    pub struct CmdOpCode : u64 {
        /// The idle command has no effect
        const IDLE        = 0x0;
        /// Sends a message
        const SEND        = 0x1;
        /// Replies to a message
        const REPLY       = 0x2;
        /// Reads from external memory
        const READ        = 0x3;
        /// Writes to external memory
        const WRITE       = 0x4;
        /// Fetches a message
        const FETCH_MSG   = 0x5;
        /// Acknowledges a message
        const ACK_MSG     = 0x6;
        /// Acknowledges events
        const ACK_EVENTS  = 0x7;
        /// Puts the CU to sleep
        const SLEEP       = 0x8;
        /// Prints a message
        const PRINT       = 0x9;
    }
}

int_enum! {
    struct EventType : u64 {
        const MSG_RECV    = 0x0;
        const CRD_RECV    = 0x1;
        const EP_INVAL    = 0x2;
    }
}

bitflags! {
    struct EventMask : u64 {
        const MSG_RECV    = 1 << EventType::MSG_RECV.val;
        const CRD_RECV    = 1 << EventType::CRD_RECV.val;
        const EP_INVAL    = 1 << EventType::EP_INVAL.val;
    }
}

bitflags! {
    /// The command flags
    pub struct CmdFlags : u64 {
        /// Specifies that a page fault should abort the command with an error
        const NOPF        = 0x1;
    }
}

int_enum! {
    /// The different endpoint types
    pub struct EpType : u64 {
        /// Invalid endpoint (unusable)
        const INVALID     = 0x0;
        /// Send endpoint
        const SEND        = 0x1;
        /// Receive endpoint
        const RECEIVE     = 0x2;
        /// Memory endpoint
        const MEMORY      = 0x3;
    }
}

int_enum! {
    /// The external requests
    pub struct ExtReqOpCode : Reg {
        /// Invalidates a TLB entry in the CU's MMU
        const INV_PAGE    = 0x0;
        /// Requests some PEMux action
        const PEMUX       = 0x1;
        /// Stops the current VPE
        const STOP        = 0x2;
    }
}

int_enum! {
    /// The external commands
    pub struct ExtCmdOpCode : Reg {
        /// The idle command has no effect
        const IDLE        = 0;
        /// Wake up the CU in case it's sleeping
        const WAKEUP_CORE = 1;
        /// Invalidate and endpoint, if possible
        const INV_EP      = 2;
        /// Invalidate a single TLB entry
        const INV_PAGE    = 3;
        /// Invalidate all TLB entries
        const INV_TLB     = 4;
        /// Invalidate replies from a given sender
        const INV_REPLY   = 5;
        /// Reset the CU
        const RESET       = 6;
        /// Acknowledge a message
        const ACK_MSG     = 7;
    }
}

pub type PTE = u64;

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
        /// Unsupported by DTU, but used for MMU
        const UNCACHED      = 0b0010_0000;
        /// Read+write
        const RW            = Self::R.bits | Self::W.bits;
        /// Read+write+execute
        const RWX           = Self::R.bits | Self::W.bits | Self::X.bits;
        /// Internal+read+write+execute
        const IRWX          = Self::R.bits | Self::W.bits | Self::X.bits | Self::I.bits;
    }
}

/// The DTU header including the reply label
#[repr(C, packed)]
#[derive(Copy, Clone, Default, Debug)]
pub struct ReplyHeader {
    pub flags: u8, // if bit 0 is set its a reply, if bit 1 is set we grant credits
    pub sender_pe: u8,
    pub sender_ep: u8,
    pub reply_ep: u8, // for a normal message this is the reply epId
    // for a reply this is the enpoint that receives credits
    pub length: u16,
    // we keep that for now, because otherwise ReplyHeader is not 16 bytes = 2 registers
    pub _reserved: u16,

    pub reply_label: u64,
}

/// The DTU header excluding the reply label
#[repr(C, packed)]
#[derive(Copy, Clone, Default, Debug)]
pub struct Header {
    pub flags: u8,
    pub sender_pe: u8,
    pub sender_ep: u8,
    pub reply_ep: u8,

    pub length: u16,
    pub _reserved: u16,

    pub reply_label: u64,
    pub label: u64,
}

/// The DTU message consisting of the header and the payload
#[repr(C, packed)]
#[derive(Debug)]
pub struct Message {
    pub header: Header,
    pub data: [u8],
}

/// The DTU interface
pub struct DTU {}

impl DTU {
    /// Sends `msg[0..size]` via given endpoint.
    ///
    /// The `reply_ep` specifies the endpoint the reply is sent to. The label of the reply will be
    /// `reply_lbl`.
    ///
    /// # Errors
    ///
    /// If the number of left credits is not sufficient, the function returns (`Code::MISS_CREDITS`).
    /// If the receiver is suspended, the function returns (`Code::VPE_GONE`).
    #[inline(always)]
    pub fn send(
        ep: EpId,
        msg: *const u8,
        size: usize,
        reply_lbl: Label,
        reply_ep: EpId,
    ) -> Result<(), Error> {
        Self::write_cmd_reg(CmdReg::DATA, Self::build_data(msg, size));
        if reply_lbl != 0 {
            Self::write_cmd_reg(CmdReg::REPLY_LABEL, reply_lbl);
        }
        Self::write_cmd_reg(
            CmdReg::COMMAND,
            Self::build_cmd(ep, CmdOpCode::SEND, 0, reply_ep as Reg),
        );

        Self::get_error()
    }

    /// Sends `reply[0..size]` as reply to `msg`.
    ///
    /// # Errors
    ///
    /// If the receiver is suspended, the function returns (`Code::VPE_GONE`).
    #[inline(always)]
    pub fn reply(
        ep: EpId,
        reply: *const u8,
        size: usize,
        msg: &'static Message,
    ) -> Result<(), Error> {
        Self::write_cmd_reg(CmdReg::DATA, Self::build_data(reply, size));
        let msg_addr = msg as *const Message as *const u8 as usize;
        Self::write_cmd_reg(
            CmdReg::COMMAND,
            Self::build_cmd(ep, CmdOpCode::REPLY, 0, msg_addr as Reg),
        );

        Self::get_error()
    }

    /// Reads `size` bytes from offset `off` in the memory region denoted by the endpoint into `data`.
    ///
    /// The `flags` can be used to control whether page faults should abort the command.
    ///
    /// # Errors
    ///
    /// If the receiver is suspended, the function returns (`Code::VPE_GONE`).
    pub fn read(
        ep: EpId,
        data: *mut u8,
        size: usize,
        off: goff,
        flags: CmdFlags,
    ) -> Result<(), Error> {
        let cmd = Self::build_cmd(ep, CmdOpCode::READ, flags.bits(), 0);
        let res = Self::transfer(cmd, data as usize, size, off);
        unsafe { intrinsics::atomic_fence() };
        res
    }

    /// Writes `size` bytes from `data` to offset `off` in the memory region denoted by the endpoint.
    ///
    /// The `flags` can be used to control whether page faults should abort the command.
    ///
    /// # Errors
    ///
    /// If the receiver is suspended, the function returns (`Code::VPE_GONE`).
    pub fn write(
        ep: EpId,
        data: *const u8,
        size: usize,
        off: goff,
        flags: CmdFlags,
    ) -> Result<(), Error> {
        let cmd = Self::build_cmd(ep, CmdOpCode::WRITE, flags.bits(), 0);
        Self::transfer(cmd, data as usize, size, off)
    }

    fn transfer(cmd: Reg, data: usize, size: usize, off: goff) -> Result<(), Error> {
        let mut left = size;
        let mut offset = off;
        let mut data_addr = data;
        while left > 0 {
            let amount = util::min(left, MAX_PKT_SIZE);
            Self::write_cmd_reg(CmdReg::DATA, data_addr as Reg | ((amount as Reg) << 48));
            Self::write_cmd_reg(CmdReg::COMMAND, cmd | ((offset as Reg) << 16));

            left -= amount;
            offset += amount as goff;
            data_addr += amount;

            Self::get_error()?;
        }
        Ok(())
    }

    /// Tries to fetch a new message from the given endpoint.
    #[inline(always)]
    pub fn fetch_msg(ep: EpId) -> Option<&'static Message> {
        Self::write_cmd_reg(
            CmdReg::COMMAND,
            Self::build_cmd(ep, CmdOpCode::FETCH_MSG, 0, 0),
        );
        unsafe { intrinsics::atomic_fence() };
        let msg = Self::read_cmd_reg(CmdReg::OFFSET);
        if msg != 0 {
            unsafe {
                let head = msg as usize as *const Header;
                let slice = [msg as usize, (*head).length as usize];
                Some(intrinsics::transmute(slice))
            }
        }
        else {
            None
        }
    }

    #[inline(always)]
    pub fn fetch_events() -> Reg {
        let old = Self::read_dtu_reg(DtuReg::EVENTS);
        if old != 0 {
            Self::write_cmd_reg(
                CmdReg::COMMAND,
                Self::build_cmd(0, CmdOpCode::ACK_EVENTS, 0, old),
            );
            unsafe { intrinsics::atomic_fence() };
        }
        old
    }

    /// Returns true if the given endpoint is valid, i.e., a SEND, RECEIVE, or MEMORY endpoint
    #[inline(always)]
    pub fn is_valid(ep: EpId) -> bool {
        let r0 = Self::read_ep_reg(ep, 0);
        (r0 >> 61) != EpType::INVALID.val
    }

    /// Returns true if the given endpoint is a SEND EP and has missing credits
    pub fn has_missing_credits(ep: EpId) -> bool {
        let r0 = Self::read_ep_reg(ep, 0);
        if (r0 >> 61) != EpType::SEND.val {
            return false;
        }
        let r1 = Self::read_ep_reg(ep, 1);
        let cur = r1 & 0xFFFF;
        let max = (r1 >> 16) & 0xFFFF;
        cur < max
    }

    /// Marks the given message for receive endpoint `ep` as read
    #[inline(always)]
    pub fn mark_read(ep: EpId, msg: &Message) {
        let off = (msg as *const Message) as *const u8 as usize as Reg;
        Self::write_cmd_reg(
            CmdReg::COMMAND,
            Self::build_cmd(ep, CmdOpCode::ACK_MSG, 0, off),
        );
    }

    /// Waits until the current command is completed and returns the error, if any occurred
    #[inline(always)]
    pub fn get_error() -> Result<(), Error> {
        loop {
            let cmd = Self::read_cmd_reg(CmdReg::COMMAND);
            if (cmd & 0xF) == CmdOpCode::IDLE.val {
                let err = (cmd >> 12) & 0xF;
                return if err == 0 {
                    Ok(())
                }
                else {
                    Err(Error::from(err as u32))
                };
            }
        }
    }

    /// Puts the CU to sleep until the CU is woken up (e.g., by a message reception).
    #[inline(always)]
    pub fn sleep() -> Result<(), Error> {
        Self::sleep_for(0)
    }

    /// Puts the CU to sleep for at most `cycles` or until the CU is woken up (e.g., by a message
    /// reception).
    #[inline(always)]
    pub fn sleep_for(cycles: u64) -> Result<(), Error> {
        Self::write_cmd_reg(
            CmdReg::COMMAND,
            Self::build_cmd(0, CmdOpCode::SLEEP, 0, cycles),
        );
        Self::get_error()
    }

    /// Prints the given message into the gem5 log
    pub fn print(s: &[u8]) {
        let regs = DTU_REGS + CMD_REGS + EP_REGS * EP_COUNT + HEADER_REGS * HEADER_COUNT;
        let mut buffer = BASE_ADDR + regs * 8;

        #[allow(clippy::transmute_ptr_to_ptr)]
        let rstr: &[u64] = unsafe { intrinsics::transmute(s) };
        let num = util::round_up(s.len(), 8) / 8;
        for c in rstr.iter().take(num) {
            arch::cpu::write8b(buffer, *c);
            buffer += 8;
        }

        Self::write_cmd_reg(
            CmdReg::COMMAND,
            Self::build_cmd(0, CmdOpCode::PRINT, 0, s.len() as u64),
        );
    }

    /// Aborts the current command or VPE, specified in `req`, and returns (`xfer_buf`, `cmd_reg`).
    ///
    /// The `xfer_buf` indicates the transfer buffer that was used by the aborted command
    /// The `cmd_reg` contains the value of the command register before the abort
    pub fn abort() -> (Reg, Reg) {
        // save the old value before aborting
        let mut cmd_reg = Self::read_cmd_reg(CmdReg::COMMAND);
        Self::write_cmd_reg(CmdReg::ABORT, 1);

        // wait until the abort is finished.
        match Self::get_error() {
            // command aborted; we'll retry it later
            Err(ref e) if e.code() == Code::Abort => {},
            // keep error code
            _ => {
                // if there was something running which finished after the read_cmd_reg above,
                // reset the cmd register to idle.
                if (cmd_reg & 0xF) != CmdOpCode::IDLE.val {
                    cmd_reg = CmdOpCode::IDLE.val;
                }
            },
        }

        (Self::read_cmd_reg(CmdReg::ABORT), cmd_reg)
    }

    pub fn retry(cmd: Reg) {
        Self::write_cmd_reg(CmdReg::COMMAND, cmd)
    }

    pub fn clear_irq() {
        Self::write_reg(DtuReg::CLEAR_IRQ.val as usize, 1);
    }

    pub fn get_ext_req() -> Reg {
        Self::read_req_reg(ReqReg::EXT_REQ)
    }

    pub fn set_ext_req(val: Reg) {
        Self::write_req_reg(ReqReg::EXT_REQ, val)
    }

    pub fn get_xlate_req() -> Reg {
        Self::read_req_reg(ReqReg::XLATE_REQ)
    }

    pub fn set_xlate_req(val: Reg) {
        Self::write_req_reg(ReqReg::XLATE_REQ, val)
    }

    pub fn set_xlate_resp(val: Reg) {
        Self::write_req_reg(ReqReg::XLATE_RESP, val)
    }

    pub fn read_cmd_reg(reg: CmdReg) -> Reg {
        Self::read_reg(DTU_REGS + reg.val as usize)
    }

    pub fn write_cmd_reg(reg: CmdReg, val: Reg) {
        Self::write_reg(DTU_REGS + reg.val as usize, val)
    }

    pub fn get_pfep() -> Reg {
        Self::read_dtu_reg(DtuReg::PF_EP)
    }

    fn read_dtu_reg(reg: DtuReg) -> Reg {
        Self::read_reg(reg.val as usize)
    }

    fn read_req_reg(reg: ReqReg) -> Reg {
        Self::read_reg((cfg::PAGE_SIZE / util::size_of::<Reg>()) + reg.val as usize)
    }

    fn read_ep_reg(ep: EpId, reg: usize) -> Reg {
        Self::read_reg(DTU_REGS + CMD_REGS + EP_REGS * ep + reg)
    }

    fn write_req_reg(reg: ReqReg, val: Reg) {
        Self::write_reg(
            (cfg::PAGE_SIZE / util::size_of::<Reg>()) + reg.val as usize,
            val,
        )
    }

    fn read_reg(idx: usize) -> Reg {
        arch::cpu::read8b(BASE_ADDR + idx * 8)
    }

    fn write_reg(idx: usize, val: Reg) {
        arch::cpu::write8b(BASE_ADDR + idx * 8, val);
    }

    fn build_data(addr: *const u8, size: usize) -> Reg {
        addr as Reg | (size as Reg) << 48
    }

    fn build_cmd(ep: EpId, c: CmdOpCode, flags: Reg, arg: Reg) -> Reg {
        c.val as Reg | ((ep as Reg) << 4) | (flags << 11) | (arg << 16)
    }
}
