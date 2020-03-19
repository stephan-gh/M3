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
use core::mem;
use errors::{Code, Error};
use goff;
use kif::PageFlags;
use math;
use util;

/// A TCU register
pub type Reg = u64;
/// An endpoint id
pub type EpId = usize;
/// A TCU label used in send EPs
pub type Label = u32;
/// A PE id
pub type PEId = usize;

/// The number of endpoints in each TCU
pub const EP_COUNT: EpId = 192;

/// The send EP for kernel calls from PEMux
pub const KPEX_SEP: EpId = 0;
/// The receive EP for kernel calls from PEMux
pub const KPEX_REP: EpId = 1;
/// The receive EP for upcalls from the kernel for PEMux
pub const PEXUP_REP: EpId = 2;
/// The reply EP for upcalls from the kernel for PEMux
pub const PEXUP_RPLEP: EpId = 3;

/// The send EP offset for system calls
pub const SYSC_SEP_OFF: EpId = 0;
/// The receive EP offset for system calls
pub const SYSC_REP_OFF: EpId = 1;
/// The receive EP offset for upcalls from the kernel
pub const UPCALL_REP_OFF: EpId = 2;
/// The reply EP offset for upcalls from the kernel
pub const UPCALL_RPLEP_OFF: EpId = 3;
/// The default receive EP offset
pub const DEF_REP_OFF: EpId = 4;
/// The pager send EP offset
pub const PG_SEP_OFF: EpId = 5;
/// The pager receive EP offset
pub const PG_REP_OFF: EpId = 6;

/// The offset of the first user EP
pub const FIRST_USER_EP: EpId = 4;
/// The number of standard EPs
pub const STD_EPS_COUNT: usize = 7;

/// The reply EP for messages that want to disable replies
pub const NO_REPLIES: EpId = 0xFFFF;

/// The base address of the TCU's MMIO area
pub const MMIO_ADDR: usize = 0xF000_0000;
/// The size of the TCU's MMIO area
pub const MMIO_SIZE: usize = cfg::PAGE_SIZE * 2;
/// The base address of the TCU's private MMIO area
pub const MMIO_PRIV_ADDR: usize = MMIO_ADDR + MMIO_SIZE;
/// The size of the TCU's private MMIO area
pub const MMIO_PRIV_SIZE: usize = cfg::PAGE_SIZE;

/// The number of TCU registers
pub const TCU_REGS: usize = 4;
/// The number of command registers
pub const CMD_REGS: usize = 4;
/// The number of registers per EP
pub const EP_REGS: usize = 3;

int_enum! {
    /// The TCU registers
    pub struct TCUReg : Reg {
        /// Stores various status flags
        const STATUS        = 0;
        const CUR_TIME      = 1;
        const CLEAR_IRQ     = 2;
        const CLOCK         = 3;
    }
}

#[allow(dead_code)]
bitflags! {
    /// The status flag for the `TCUReg::STATUS` register
    pub struct StatusFlags : Reg {
        /// Whether the PE is privileged
        const PRIV          = 1 << 0;
    }
}

#[allow(dead_code)]
int_enum! {
    /// The privileged registers
    pub struct PrivReg : Reg {
        /// For core requests
        const CORE_REQ      = 0x0;
        /// For core responses
        const CORE_RESP     = 0x1;
        /// For privileged commands
        const PRIV_CMD      = 0x2;
        /// The argument for privileged commands
        const PRIV_CMD_ARG  = 0x3;
        /// For external commands
        const EXT_CMD       = 0x4;
        /// The current VPE
        const CUR_VPE       = 0x5;
        /// The old VPE (only set by XCHG_VPE command)
        const OLD_VPE       = 0x6;
    }
}

#[allow(dead_code)]
int_enum! {
    /// The command registers
    pub struct CmdReg : Reg {
        /// Starts commands and signals their completion
        const COMMAND       = 0x0;
        /// Aborts commands
        const ABORT         = 0x1;
        /// Specifies the data address and size
        const DATA          = 0x2;
        /// Specifies an additional argument
        const ARG1          = 0x3;
    }
}

int_enum! {
    /// The commands
    pub struct CmdOpCode : u64 {
        /// The idle command has no effect
        const IDLE          = 0x0;
        /// Sends a message
        const SEND          = 0x1;
        /// Replies to a message
        const REPLY         = 0x2;
        /// Reads from external memory
        const READ          = 0x3;
        /// Writes to external memory
        const WRITE         = 0x4;
        /// Fetches a message
        const FETCH_MSG     = 0x5;
        /// Fetches the events
        const FETCH_EVENTS  = 0x6;
        /// Acknowledges a message
        const ACK_MSG       = 0x7;
        /// Puts the CU to sleep
        const SLEEP         = 0x8;
        /// Prints a message
        const PRINT         = 0x9;
    }
}

int_enum! {
    struct EventType : u64 {
        const CRD_RECV      = 0x0;
        const EP_INVAL      = 0x1;
        const USER          = 0x2;
    }
}

bitflags! {
    pub struct EventMask : u64 {
        const CRD_RECV      = 1 << EventType::CRD_RECV.val;
        const EP_INVAL      = 1 << EventType::EP_INVAL.val;
        const USER          = 1 << EventType::USER.val;
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
    /// The privileged commands
    pub struct PrivCmdOpCode : Reg {
        /// The idle command has no effect
        const IDLE        = 0;
        /// Invalidate a single TLB entry
        const INV_PAGE    = 1;
        /// Invalidate all TLB entries
        const INV_TLB     = 2;
        /// Insert an entry into the TLB
        const INS_TLB     = 3;
        /// Changes the VPE
        const XCHG_VPE    = 4;
    }
}

int_enum! {
    /// The external commands
    pub struct ExtCmdOpCode : Reg {
        /// The idle command has no effect
        const IDLE        = 0;
        /// Invalidate and endpoint, if possible
        const INV_EP      = 1;
        /// Invalidate replies from a given sender
        const INV_REPLY   = 2;
        /// Reset the CU
        const RESET       = 3;
    }
}

/// The TCU header
#[repr(C, packed)]
#[derive(Copy, Clone, Default, Debug)]
pub struct Header {
    pub flags_reply_size: u8,
    pub sender_pe: u8,
    pub sender_ep: u16,
    pub reply_ep: u16,

    pub length: u16,

    pub reply_label: u32,
    pub label: u32,
}

/// The TCU message consisting of the header and the payload
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

/// The TCU interface
pub struct TCU {}

impl TCU {
    /// Sends `msg[0..size]` via given endpoint.
    ///
    /// The `reply_ep` specifies the endpoint the reply is sent to. The label of the reply will be
    /// `reply_lbl`.
    ///
    /// # Errors
    ///
    /// If the number of left credits is not sufficient, the function returns (`Code::MISS_CREDITS`).
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
            Self::write_cmd_reg(CmdReg::ARG1, reply_lbl as Reg);
        }
        Self::write_cmd_reg(
            CmdReg::COMMAND,
            Self::build_cmd(ep, CmdOpCode::SEND, 0, reply_ep as Reg),
        );

        Self::get_error()
    }

    /// Sends `reply[0..size]` as reply to `msg`.
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
    pub fn read(
        ep: EpId,
        data: *mut u8,
        size: usize,
        off: goff,
        flags: CmdFlags,
    ) -> Result<(), Error> {
        assert!(size <= 0xFFFFFFFF);
        Self::write_cmd_reg(CmdReg::DATA, Self::build_data(data, size));
        Self::write_cmd_reg(CmdReg::ARG1, off as Reg);
        Self::write_cmd_reg(
            CmdReg::COMMAND,
            Self::build_cmd(ep, CmdOpCode::READ, flags.bits(), 0),
        );
        let res = Self::get_error();
        unsafe { intrinsics::atomic_fence() };
        res
    }

    /// Writes `size` bytes from `data` to offset `off` in the memory region denoted by the endpoint.
    ///
    /// The `flags` can be used to control whether page faults should abort the command.
    pub fn write(
        ep: EpId,
        data: *const u8,
        size: usize,
        off: goff,
        flags: CmdFlags,
    ) -> Result<(), Error> {
        assert!(size <= 0xFFFFFFFF);
        Self::write_cmd_reg(CmdReg::DATA, Self::build_data(data, size));
        Self::write_cmd_reg(CmdReg::ARG1, off as Reg);
        Self::write_cmd_reg(
            CmdReg::COMMAND,
            Self::build_cmd(ep, CmdOpCode::WRITE, flags.bits(), 0),
        );
        Self::get_error()
    }

    /// Tries to fetch a new message from the given endpoint.
    #[inline(always)]
    pub fn fetch_msg(ep: EpId) -> Option<&'static Message> {
        Self::write_cmd_reg(
            CmdReg::COMMAND,
            Self::build_cmd(ep, CmdOpCode::FETCH_MSG, 0, 0),
        );
        unsafe { intrinsics::atomic_fence() };
        let msg = Self::read_cmd_reg(CmdReg::ARG1);
        if msg != 0 {
            Some(Self::addr_to_msg(msg))
        }
        else {
            None
        }
    }

    #[inline(always)]
    pub fn fetch_events() -> Reg {
        Self::write_cmd_reg(
            CmdReg::COMMAND,
            Self::build_cmd(0, CmdOpCode::FETCH_EVENTS, 0, 0),
        );
        unsafe { intrinsics::atomic_fence() };
        Self::read_cmd_reg(CmdReg::ARG1)
    }

    /// Returns true if the given endpoint is valid, i.e., a SEND, RECEIVE, or MEMORY endpoint
    #[inline(always)]
    pub fn is_valid(ep: EpId) -> bool {
        let r0 = Self::read_ep_reg(ep, 0);
        (r0 & 0x7) != EpType::INVALID.val
    }

    /// Returns true if the given endpoint is a SEND EP and has missing credits
    pub fn has_missing_credits(ep: EpId) -> bool {
        let r0 = Self::read_ep_reg(ep, 0);
        if (r0 & 0x7) != EpType::SEND.val {
            return false;
        }
        let r0 = Self::read_ep_reg(ep, 0);
        let cur = (r0 >> 19) & 0x3F;
        let max = (r0 >> 25) & 0x3F;
        cur < max
    }

    /// Marks the given message for receive endpoint `ep` as read
    #[inline(always)]
    pub fn ack_msg(ep: EpId, msg: &Message) {
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
                let err = (cmd >> 21) & 0xF;
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
        Self::wait_for_msg(0xFFFF, cycles)
    }

    /// Puts the CU to sleep until a message arrives at receive EP `ep`, but at most for `cycles`.
    #[inline(always)]
    pub fn wait_for_msg(ep: EpId, cycles: u64) -> Result<(), Error> {
        Self::write_cmd_reg(CmdReg::ARG1, ((ep as Reg) << 48) | cycles as Reg);
        Self::write_cmd_reg(CmdReg::COMMAND, Self::build_cmd(0, CmdOpCode::SLEEP, 0, 0));
        Self::get_error()
    }

    /// Drops all messages in the receive buffer of given receive EP that have the given label.
    pub fn drop_msgs_with(ep: EpId, label: Label) {
        // we assume that the one that used the label can no longer send messages. thus, if there
        // are no messages yet, we are done.
        let unread = Self::read_ep_reg(ep, 2) >> 32;
        if unread == 0 {
            return;
        }

        let r0 = Self::read_ep_reg(ep, 0);
        let base = Self::read_ep_reg(ep, 1);
        let buf_size = 1 << ((r0 >> 35) & 0x3F);
        let msg_size = (r0 >> 41) & 0x3F;
        for i in 0..buf_size {
            if (unread & (1 << i)) != 0 {
                let msg = Self::addr_to_msg(base + (i << msg_size));
                if msg.header.label == label {
                    Self::ack_msg(ep, msg);
                }
            }
        }
    }

    /// Prints the given message into the gem5 log
    pub fn print(s: &[u8]) {
        let regs = TCU_REGS + CMD_REGS + EP_REGS * EP_COUNT;
        let mut buffer = MMIO_ADDR + regs * 8;

        #[allow(clippy::transmute_ptr_to_ptr)]
        let rstr: &[u64] = unsafe { intrinsics::transmute(s) };
        let num = math::round_up(s.len(), 8) / 8;
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
        // ensure that we read the command register before the abort has been executed
        unsafe { intrinsics::atomic_fence() };
        Self::write_cmd_reg(CmdReg::ABORT, 1);
        // ensure that we read the command register after the abort has been executed
        unsafe { intrinsics::atomic_fence() };

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
        Self::write_reg(TCUReg::CLEAR_IRQ.val as usize, 1);
    }

    pub fn get_core_req() -> Reg {
        Self::read_priv_reg(PrivReg::CORE_REQ)
    }

    pub fn set_core_req(val: Reg) {
        Self::write_priv_reg(PrivReg::CORE_REQ, val)
    }

    pub fn set_core_resp(val: Reg) {
        Self::write_priv_reg(PrivReg::CORE_RESP, val)
    }

    pub fn get_cur_vpe() -> Reg {
        Self::read_priv_reg(PrivReg::CUR_VPE)
    }

    pub fn xchg_vpe(nvpe: Reg) -> Reg {
        Self::write_priv_reg(PrivReg::PRIV_CMD, PrivCmdOpCode::XCHG_VPE.val | (nvpe << 4));
        unsafe { intrinsics::atomic_fence() };
        Self::read_priv_reg(PrivReg::OLD_VPE)
    }

    pub fn invalidate_tlb() {
        Self::write_priv_reg(PrivReg::PRIV_CMD, PrivCmdOpCode::INV_TLB.val);
    }

    pub fn invalidate_page(asid: u16, virt: usize) {
        let val = ((asid as Reg) << 48) | ((virt as Reg) << 4) | PrivCmdOpCode::INV_PAGE.val;
        Self::write_priv_reg(PrivReg::PRIV_CMD, val);
    }

    pub fn insert_tlb(asid: u16, virt: usize, phys: u64, flags: PageFlags) {
        Self::write_priv_reg(PrivReg::PRIV_CMD_ARG, phys);
        unsafe { intrinsics::atomic_fence() };
        let cmd = ((asid as Reg) << 48)
            | (((virt & !cfg::PAGE_MASK) as Reg) << 4)
            | ((flags.bits() as Reg) << 4)
            | PrivCmdOpCode::INS_TLB.val;
        Self::write_priv_reg(PrivReg::PRIV_CMD, cmd);
    }

    pub fn read_cmd_reg(reg: CmdReg) -> Reg {
        Self::read_reg(TCU_REGS + reg.val as usize)
    }

    pub fn write_cmd_reg(reg: CmdReg, val: Reg) {
        Self::write_reg(TCU_REGS + reg.val as usize, val)
    }

    fn read_priv_reg(reg: PrivReg) -> Reg {
        Self::read_reg(((cfg::PAGE_SIZE * 2) / util::size_of::<Reg>()) + reg.val as usize)
    }

    fn read_ep_reg(ep: EpId, reg: usize) -> Reg {
        Self::read_reg(TCU_REGS + CMD_REGS + EP_REGS * ep + reg)
    }

    fn write_priv_reg(reg: PrivReg, val: Reg) {
        Self::write_reg(
            ((cfg::PAGE_SIZE * 2) / util::size_of::<Reg>()) + reg.val as usize,
            val,
        )
    }

    fn addr_to_msg(addr: Reg) -> &'static Message {
        // safety: the cast is okay because we trust the TCU
        unsafe {
            let head = addr as usize as *const Header;
            let slice = [addr as usize, (*head).length as usize];
            intrinsics::transmute(slice)
        }
    }

    fn read_reg(idx: usize) -> Reg {
        arch::cpu::read8b(MMIO_ADDR + idx * 8)
    }

    fn write_reg(idx: usize, val: Reg) {
        arch::cpu::write8b(MMIO_ADDR + idx * 8, val);
    }

    fn build_data(addr: *const u8, size: usize) -> Reg {
        addr as Reg | (size as Reg) << 32
    }

    fn build_cmd(ep: EpId, c: CmdOpCode, flags: Reg, arg: Reg) -> Reg {
        c.val as Reg | ((ep as Reg) << 4) | (flags << 20) | (arg << 25)
    }
}
