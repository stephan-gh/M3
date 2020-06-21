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
use errors::Error;
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

/// An invalid endpoint ID
pub const INVALID_EP: EpId = 0xFFFF;
/// The reply EP for messages that want to disable replies
pub const NO_REPLIES: EpId = INVALID_EP;
/// Represents unlimited credits for send EPs
pub const UNLIM_CREDITS: u32 = 0x3F;

/// The base address of the TCU's MMIO area
pub const MMIO_ADDR: usize = 0xF000_0000;
/// The size of the TCU's MMIO area
pub const MMIO_SIZE: usize = cfg::PAGE_SIZE * 2;
/// The base address of the TCU's private MMIO area
pub const MMIO_PRIV_ADDR: usize = MMIO_ADDR + MMIO_SIZE;
/// The size of the TCU's private MMIO area
pub const MMIO_PRIV_SIZE: usize = cfg::PAGE_SIZE;

/// The number of external registers
pub const EXT_REGS: usize = 2;
/// The number of unprivileged registers
pub const UNPRIV_REGS: usize = 5;
/// The number of registers per EP
pub const EP_REGS: usize = 3;

int_enum! {
    /// The external registers
    pub struct ExtReg : Reg {
        /// Stores the privileged flag (for now)
        const FEATURES      = 0x0;
        /// For external commands
        const EXT_CMD       = 0x1;
    }
}

#[allow(dead_code)]
bitflags! {
    /// The status flag for the [`ExtReg::FEATURES`] register
    pub struct FeatureFlags : Reg {
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
        /// For privileged commands
        const PRIV_CMD      = 0x1;
        /// The argument for privileged commands
        const PRIV_CMD_ARG  = 0x2;
        /// The current VPE
        const CUR_VPE       = 0x3;
        /// The old VPE (only set by XCHG_VPE command)
        const OLD_VPE       = 0x4;
        /// Used to ack IRQ requests
        const CLEAR_IRQ     = 0x5;
    }
}

#[allow(dead_code)]
int_enum! {
    /// The unprivileged registers
    pub struct UnprivReg : Reg {
        /// Starts commands and signals their completion
        const COMMAND       = 0x0;
        /// Specifies the data address and size
        const DATA          = 0x1;
        /// Specifies an additional argument
        const ARG1          = 0x2;
        /// The current time in nanoseconds
        const CUR_TIME      = 0x3;
        /// Prints a line into the gem5 log
        const PRINT         = 0x4;
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
        /// Acknowledges a message
        const ACK_MSG       = 0x6;
        /// Puts the CU to sleep
        const SLEEP         = 0x7;
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
        /// Sets the timer
        const SET_TIMER   = 5;
        /// Abort the current command
        const ABORT_CMD   = 6;
        /// Flushes and invalidates the cache
        const FLUSH_CACHE = 7;
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

int_enum! {
    /// The TCU-internal IRQ ids to clear IRQs
    pub struct IRQ : Reg {
        /// The core request IRQ
        const CORE_REQ  = 0;
        /// The timer IRQ
        const TIMER     = 1;
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
    /// If the number of left credits is not sufficient, the function returns
    /// [`MissCredits`](::errors::Code::MissCredits).
    #[inline(always)]
    pub fn send(
        ep: EpId,
        msg: *const u8,
        size: usize,
        reply_lbl: Label,
        reply_ep: EpId,
    ) -> Result<(), Error> {
        Self::write_unpriv_reg(UnprivReg::DATA, Self::build_data(msg, size));
        if reply_lbl != 0 {
            Self::write_unpriv_reg(UnprivReg::ARG1, reply_lbl as Reg);
        }
        Self::write_unpriv_reg(
            UnprivReg::COMMAND,
            Self::build_cmd(ep, CmdOpCode::SEND, reply_ep as Reg),
        );

        Self::get_error()
    }

    /// Sends `reply[0..size]` as reply to `msg`.
    #[inline(always)]
    pub fn reply(ep: EpId, reply: *const u8, size: usize, msg_off: usize) -> Result<(), Error> {
        Self::write_unpriv_reg(UnprivReg::DATA, Self::build_data(reply, size));
        Self::write_unpriv_reg(
            UnprivReg::COMMAND,
            Self::build_cmd(ep, CmdOpCode::REPLY, msg_off as Reg),
        );

        Self::get_error()
    }

    /// Reads `size` bytes from offset `off` in the memory region denoted by the endpoint into `data`.
    pub fn read(ep: EpId, data: *mut u8, size: usize, off: goff) -> Result<(), Error> {
        Self::write_unpriv_reg(UnprivReg::DATA, Self::build_data(data, size));
        Self::write_unpriv_reg(UnprivReg::ARG1, off as Reg);
        Self::write_unpriv_reg(UnprivReg::COMMAND, Self::build_cmd(ep, CmdOpCode::READ, 0));
        let res = Self::get_error();
        unsafe { intrinsics::atomic_fence() };
        res
    }

    /// Writes `size` bytes from `data` to offset `off` in the memory region denoted by the endpoint.
    pub fn write(ep: EpId, data: *const u8, size: usize, off: goff) -> Result<(), Error> {
        Self::write_unpriv_reg(UnprivReg::DATA, Self::build_data(data, size));
        Self::write_unpriv_reg(UnprivReg::ARG1, off as Reg);
        Self::write_unpriv_reg(UnprivReg::COMMAND, Self::build_cmd(ep, CmdOpCode::WRITE, 0));
        Self::get_error()
    }

    /// Tries to fetch a new message from the given endpoint.
    #[inline(always)]
    pub fn fetch_msg(ep: EpId) -> Option<usize> {
        Self::write_unpriv_reg(
            UnprivReg::COMMAND,
            Self::build_cmd(ep, CmdOpCode::FETCH_MSG, 0),
        );
        Self::get_error().ok()?;
        let msg = Self::read_unpriv_reg(UnprivReg::ARG1);
        if msg != !0 {
            Some(msg as usize)
        }
        else {
            None
        }
    }

    /// Assuming that `ep` is a receive EP, the function returns whether there are unread messages.
    #[inline(always)]
    pub fn has_msgs(ep: EpId) -> bool {
        let r2 = Self::read_ep_reg(ep, 2);
        (r2 >> 32) != 0
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
        let cur = (r0 >> 19) & 0x3F;
        let max = (r0 >> 25) & 0x3F;
        cur < max
    }

    /// Marks the given message for receive endpoint `ep` as read
    #[inline(always)]
    pub fn ack_msg(ep: EpId, msg_off: usize) -> Result<(), Error> {
        // ensure that we are really done with the message before acking it
        unsafe { intrinsics::atomic_fence() };
        Self::write_unpriv_reg(
            UnprivReg::COMMAND,
            Self::build_cmd(ep, CmdOpCode::ACK_MSG, msg_off as Reg),
        );
        Self::get_error()
    }

    /// Waits until the current command is completed and returns the error, if any occurred
    #[inline(always)]
    pub fn get_error() -> Result<(), Error> {
        loop {
            let cmd = Self::read_unpriv_reg(UnprivReg::COMMAND);
            if (cmd & 0xF) == CmdOpCode::IDLE.val {
                let err = (cmd >> 20) & 0xF;
                return if err == 0 {
                    Ok(())
                }
                else {
                    Err(Error::from(err as u32))
                };
            }
        }
    }

    /// Returns the time in nanoseconds since boot
    #[inline(always)]
    pub fn nanotime() -> u64 {
        Self::read_unpriv_reg(UnprivReg::CUR_TIME)
    }

    /// Puts the CU to sleep until the CU is woken up (e.g., by a message reception).
    #[inline(always)]
    pub fn sleep() -> Result<(), Error> {
        Self::wait_for_msg(INVALID_EP)
    }

    /// Puts the CU to sleep until a message arrives at receive EP `ep`.
    #[inline(always)]
    pub fn wait_for_msg(ep: EpId) -> Result<(), Error> {
        Self::write_unpriv_reg(
            UnprivReg::COMMAND,
            Self::build_cmd(0, CmdOpCode::SLEEP, ep as u64),
        );
        Self::get_error()
    }

    /// Drops all messages in the receive buffer of given receive EP that have the given label.
    pub fn drop_msgs_with(buf_addr: usize, ep: EpId, label: Label) {
        // we assume that the one that used the label can no longer send messages. thus, if there
        // are no messages yet, we are done.
        let unread = Self::read_ep_reg(ep, 2) >> 32;
        if unread == 0 {
            return;
        }

        let r0 = Self::read_ep_reg(ep, 0);
        let buf_size = 1 << ((r0 >> 35) & 0x3F);
        let msg_size = (r0 >> 41) & 0x3F;
        for i in 0..buf_size {
            if (unread & (1 << i)) != 0 {
                let msg = Self::offset_to_msg(buf_addr, i << msg_size);
                if msg.header.label == label {
                    Self::ack_msg(ep, (i << msg_size) as usize).ok();
                }
            }
        }
    }

    /// Prints the given message into the gem5 log
    pub fn print(s: &[u8]) {
        let regs = EXT_REGS + UNPRIV_REGS + EP_REGS * EP_COUNT;
        let mut buffer = MMIO_ADDR + regs * 8;

        #[allow(clippy::transmute_ptr_to_ptr)]
        let rstr: &[u64] = unsafe { intrinsics::transmute(s) };
        let num = math::round_up(s.len(), 8) / 8;
        for c in rstr.iter().take(num) {
            // safety: we know that the address is within the MMIO region of the TCU
            unsafe { arch::cpu::write8b(buffer, *c) };
            buffer += 8;
        }

        Self::write_unpriv_reg(UnprivReg::PRINT, s.len() as u64);
    }

    /// Aborts the current command or VPE, specified in `req`, and returns the command register to
    /// use for a retry later.
    pub fn abort_cmd() -> Reg {
        // save the old value before aborting
        let cmd_reg = Self::read_unpriv_reg(UnprivReg::COMMAND);
        // ensure that we read the command register before the abort has been executed
        unsafe { intrinsics::atomic_fence() };
        Self::write_priv_reg(PrivReg::PRIV_CMD, PrivCmdOpCode::ABORT_CMD.val);

        loop {
            let cmd = Self::read_priv_reg(PrivReg::PRIV_CMD);
            if (cmd & 0xF) == PrivCmdOpCode::IDLE.val {
                return if (cmd >> 4) == 0 {
                    // if the command was finished successfully, use the current command register
                    // to ensure that we don't forget the error code
                    Self::read_unpriv_reg(UnprivReg::COMMAND)
                }
                else {
                    // otherwise use the old one to repeat it later
                    cmd_reg
                };
            }
        }
    }

    /// Translates the offset `off` to the message address, using `base` as the base address of the
    /// message's receive buffer
    pub fn offset_to_msg(base: usize, off: usize) -> &'static Message {
        // safety: the cast is okay because we trust the TCU
        unsafe {
            let head = (base + off) as *const Header;
            let slice = [base + off, (*head).length as usize];
            intrinsics::transmute(slice)
        }
    }

    /// Translates the message address `msg` to the offset within its receive buffer, using `base`
    /// as the base address of the receive buffer
    pub fn msg_to_offset(base: usize, msg: &Message) -> usize {
        let addr = msg as *const _ as *const u8 as usize;
        addr - base
    }

    /// Returns the injected IRQ (assuming that a IRQ has been injected and was not cleared yet)
    pub fn get_irq() -> IRQ {
        IRQ::from(Self::read_priv_reg(PrivReg::CLEAR_IRQ))
    }

    /// Clears the given IRQ to notify the TCU that the IRQ has been accepted
    pub fn clear_irq(irq: IRQ) {
        Self::write_priv_reg(PrivReg::CLEAR_IRQ, irq.val);
    }

    /// Returns the current core request
    pub fn get_core_req() -> Reg {
        Self::read_priv_reg(PrivReg::CORE_REQ)
    }

    /// Sets the response for the current core request to `val`
    pub fn set_core_req(val: Reg) {
        Self::write_priv_reg(PrivReg::CORE_REQ, val)
    }

    /// Returns the current VPE with its id and message count
    pub fn get_cur_vpe() -> Reg {
        Self::read_priv_reg(PrivReg::CUR_VPE)
    }

    /// Switches to the given VPE and returns the old VPE
    pub fn xchg_vpe(nvpe: Reg) -> Reg {
        Self::write_priv_reg(PrivReg::PRIV_CMD, PrivCmdOpCode::XCHG_VPE.val | (nvpe << 4));
        unsafe { intrinsics::atomic_fence() };
        Self::read_priv_reg(PrivReg::OLD_VPE)
    }

    /// Invalidates the TCU's TLB
    pub fn invalidate_tlb() {
        Self::write_priv_reg(PrivReg::PRIV_CMD, PrivCmdOpCode::INV_TLB.val);
    }

    /// Invalidates the entry with given address space id and virtual address in the TCU's TLB
    pub fn invalidate_page(asid: u16, virt: usize) {
        let val = ((asid as Reg) << 36) | ((virt as Reg) << 4) | PrivCmdOpCode::INV_PAGE.val;
        Self::write_priv_reg(PrivReg::PRIV_CMD, val);
    }

    /// Inserts the given entry into the TCU's TLB
    pub fn insert_tlb(asid: u16, virt: usize, phys: u64, flags: PageFlags) {
        Self::write_priv_reg(PrivReg::PRIV_CMD_ARG, phys);
        unsafe {
            intrinsics::atomic_fence()
        };
        let cmd = ((asid as Reg) << 36)
            | (((virt & !cfg::PAGE_MASK) as Reg) << 4)
            | ((flags.bits() as Reg) << 4)
            | PrivCmdOpCode::INS_TLB.val;
        Self::write_priv_reg(PrivReg::PRIV_CMD, cmd);
    }

    /// Flushes and invalidates the CPU caches
    pub fn flush_cache() {
        Self::write_priv_reg(PrivReg::PRIV_CMD, PrivCmdOpCode::FLUSH_CACHE.val);
    }

    /// Sets the timer to fire in `delay_ns` nanoseconds if `delay_ns` is nonzero. Otherwise, unsets
    /// the timer.
    pub fn set_timer(delay_ns: u64) {
        Self::write_priv_reg(
            PrivReg::PRIV_CMD,
            PrivCmdOpCode::SET_TIMER.val | (delay_ns << 4),
        );
    }

    /// Returns the value of the given unprivileged register
    pub fn read_unpriv_reg(reg: UnprivReg) -> Reg {
        Self::read_reg(EXT_REGS + reg.val as usize)
    }

    /// Sets the value of the given unprivileged register to `val`
    pub fn write_unpriv_reg(reg: UnprivReg, val: Reg) {
        Self::write_reg(EXT_REGS + reg.val as usize, val)
    }

    fn read_ep_reg(ep: EpId, reg: usize) -> Reg {
        Self::read_reg(EXT_REGS + UNPRIV_REGS + EP_REGS * ep + reg)
    }

    fn read_priv_reg(reg: PrivReg) -> Reg {
        Self::read_reg(((cfg::PAGE_SIZE * 2) / util::size_of::<Reg>()) + reg.val as usize)
    }

    fn write_priv_reg(reg: PrivReg, val: Reg) {
        Self::write_reg(
            ((cfg::PAGE_SIZE * 2) / util::size_of::<Reg>()) + reg.val as usize,
            val,
        )
    }

    fn read_reg(idx: usize) -> Reg {
        // safety: we know that the address is within the MMIO region of the TCU
        unsafe { arch::cpu::read8b(MMIO_ADDR + idx * 8) }
    }

    fn write_reg(idx: usize, val: Reg) {
        // safety: as above
        unsafe { arch::cpu::write8b(MMIO_ADDR + idx * 8, val) };
    }

    fn build_data(addr: *const u8, size: usize) -> Reg {
        addr as Reg | (size as Reg) << 32
    }

    fn build_cmd(ep: EpId, c: CmdOpCode, arg: Reg) -> Reg {
        c.val as Reg | ((ep as Reg) << 4) | (arg << 24)
    }
}

#[cfg(feature = "kernel")]
impl TCU {
    /// Configures the given endpoint
    pub fn set_ep_regs(ep: EpId, regs: &[Reg]) {
        let off = EXT_REGS + UNPRIV_REGS + EP_REGS * ep;
        let addr = MMIO_ADDR + off * 8;
        for i in 0..EP_REGS {
            unsafe {
                arch::cpu::write8b(addr + i * util::size_of::<Reg>(), regs[i]);
            }
        }
    }

    /// Returns the MMIO address for the given external register
    pub fn ext_reg_addr(reg: ExtReg) -> usize {
        MMIO_ADDR + reg.val as usize * 8
    }

    /// Returns the MMIO address of the given endpoint registers
    pub fn ep_regs_addr(ep: EpId) -> usize {
        MMIO_ADDR + (EXT_REGS + UNPRIV_REGS + EP_REGS * ep) * 8
    }
}
