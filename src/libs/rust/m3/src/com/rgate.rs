/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

use core::fmt;
use core::ops;

use crate::arch;
use crate::cap::{CapFlags, Selector};
use crate::cell::LazyReadOnlyCell;
use crate::cfg;
use crate::com::rbufs::{alloc_rbuf, free_rbuf};
use crate::com::{gate::Gate, RecvBuf, SendGate};
use crate::errors::{Code, Error};
use crate::kif::INVALID_SEL;
use crate::math;
use crate::mem::MsgBuf;
use crate::syscalls;
use crate::tcu;
use crate::tiles::Activity;

const DEF_MSG_ORD: u32 = 6;

static SYS_RGATE: LazyReadOnlyCell<RecvGate> = LazyReadOnlyCell::default();
static UPC_RGATE: LazyReadOnlyCell<RecvGate> = LazyReadOnlyCell::default();
static DEF_RGATE: LazyReadOnlyCell<RecvGate> = LazyReadOnlyCell::default();

/// A receive gate (`RecvGate`) can receive messages via TCU from connected [`SendGate`]s and can
/// reply on the received messages.
pub struct RecvGate {
    gate: Gate,
    buf: Option<RecvBuf>,
    buf_addr: Option<usize>,
    order: u32,
    msg_order: u32,
}

impl fmt::Debug for RecvGate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "RecvGate[sel: {}, buf: {:?}, size: {:#0x}, ep: {:?}]",
            self.sel(),
            self.buf,
            1 << self.order,
            self.gate.epid()
        )
    }
}

/// The arguments for `RecvGate` creations
pub struct RGateArgs {
    order: u32,
    msg_order: u32,
    sel: Selector,
    flags: CapFlags,
}

impl Default for RGateArgs {
    fn default() -> Self {
        RGateArgs {
            order: DEF_MSG_ORD,
            msg_order: DEF_MSG_ORD,
            sel: INVALID_SEL,
            flags: CapFlags::empty(),
        }
    }
}

impl RGateArgs {
    /// Sets the size of the receive buffer as a power of two. That is, the size in bytes is
    /// `2^order`. This overwrites the default size of 64 bytes.
    pub fn order(mut self, order: u32) -> Self {
        self.order = order;
        self
    }

    /// Sets the size of message slots in the receive buffer as a power of two. That is, the size in
    /// bytes is `2^order`. This overwrites the default size of 64 bytes.
    pub fn msg_order(mut self, msg_order: u32) -> Self {
        self.msg_order = msg_order;
        self
    }

    /// Sets the capability selector to use for the `RecvGate`. Otherwise and by default,
    /// [`Activity::own().alloc_sel`](crate::tiles::OwnActivity::alloc_sel) will be used.
    pub fn sel(mut self, sel: Selector) -> Self {
        self.sel = sel;
        self
    }

    /// Sets the flags to `flags`.
    pub fn flags(mut self, flags: CapFlags) -> Self {
        self.flags = flags;
        self
    }
}

impl RecvGate {
    /// Returns the receive gate to receive system call replies
    pub fn syscall() -> &'static RecvGate {
        SYS_RGATE.get()
    }

    /// Returns the receive gate to receive upcalls from the kernel
    pub fn upcall() -> &'static RecvGate {
        UPC_RGATE.get()
    }

    /// Returns the default receive gate
    pub fn def() -> &'static RecvGate {
        DEF_RGATE.get()
    }

    const fn new_def(sel: Selector, ep: tcu::EpId, addr: usize, order: u32) -> Self {
        RecvGate {
            gate: Gate::new_with_ep(sel, CapFlags::KEEP_CAP, ep),
            buf: None,
            buf_addr: Some(addr),
            order,
            msg_order: order,
        }
    }

    /// Creates a new `RecvGate` with a `2^order` bytes receive buffer and `2^msg_order` bytes
    /// message slots.
    pub fn new(order: u32, msg_order: u32) -> Result<Self, Error> {
        Self::new_with(RGateArgs::default().order(order).msg_order(msg_order))
    }

    /// Creates a new `RecvGate` with given arguments.
    pub fn new_with(args: RGateArgs) -> Result<Self, Error> {
        let sel = if args.sel == INVALID_SEL {
            Activity::own().alloc_sel()
        }
        else {
            args.sel
        };

        syscalls::create_rgate(sel, args.order, args.msg_order)?;
        Ok(RecvGate {
            gate: Gate::new(sel, args.flags),
            buf: None,
            buf_addr: None,
            order: args.order,
            msg_order: args.msg_order,
        })
    }

    /// Creates the `RecvGate` with given name as defined in the application's configuration
    pub fn new_named(name: &str) -> Result<Self, Error> {
        let sel = Activity::own().alloc_sel();
        let (order, msg_order) = Activity::own().resmng().unwrap().use_rgate(sel, name)?;
        Ok(RecvGate {
            gate: Gate::new(sel, CapFlags::empty()),
            buf: None,
            buf_addr: None,
            order,
            msg_order,
        })
    }

    /// Binds a new `RecvGate` to the given selector. The `order` argument denotes the size of the
    /// receive buffer (`2^order`) and `msg_order` denotes the size of the messages (`2^msg_order`).
    pub fn new_bind(sel: Selector, order: u32, msg_order: u32) -> Self {
        RecvGate {
            gate: Gate::new(sel, CapFlags::KEEP_CAP),
            buf: None,
            buf_addr: None,
            order,
            msg_order,
        }
    }

    /// Returns the selector of the gate
    pub fn sel(&self) -> Selector {
        self.gate.sel()
    }

    /// Returns the endpoint of the gate. If the gate is not activated, `None` is returned.
    pub(crate) fn ep(&self) -> Option<tcu::EpId> {
        self.gate.epid()
    }

    /// Returns the size of the receive buffer in bytes
    pub fn size(&self) -> usize {
        1 << self.order
    }

    /// Returns the maximum message size
    pub fn max_msg_size(&self) -> usize {
        1 << self.msg_order
    }

    /// Returns the address of the receive buffer
    pub fn address(&self) -> Option<usize> {
        self.buf_addr
    }

    /// Activates this receive gate. Activation is required before [`SendGate`]s connected to this
    /// `RecvGate` can be activated.
    pub fn activate(&mut self) -> Result<(), Error> {
        if self.ep().is_none() {
            if self.buf.is_none() {
                let buf = alloc_rbuf(1 << self.order)?;
                self.buf_addr = Some(buf.addr());
                self.buf = Some(buf);
            }

            let buf = self.buf.as_ref().unwrap();
            let replies = 1 << (self.order - self.msg_order);
            self.gate.activate_rgate(buf.mem(), buf.off(), replies)?;
        }

        Ok(())
    }

    /// Activates this receive gate with given receive buffer
    pub fn activate_with(
        &mut self,
        mem: Option<Selector>,
        off: usize,
        addr: usize,
    ) -> Result<(), Error> {
        let replies = 1 << (self.order - self.msg_order);
        self.gate.activate_rgate(mem, off, replies).map(|_| {
            self.buf_addr = Some(addr);
        })
    }

    /// Deactivates this gate.
    pub fn deactivate(&mut self) {
        self.gate.release(true);
    }

    /// Returns true if there are messages that can be fetched
    pub fn has_msgs(&self) -> bool {
        tcu::TCU::has_msgs(self.ep().unwrap())
    }

    /// Tries to fetch a message from the receive gate. If there is an unread message, it returns
    /// a reference to the message. Otherwise it returns None.
    pub fn fetch(&self) -> Option<&'static tcu::Message> {
        tcu::TCU::fetch_msg(self.ep().unwrap())
            .map(|off| tcu::TCU::offset_to_msg(self.address().unwrap(), off))
    }

    /// Sends `reply` as a reply to the message `msg`.
    #[inline(always)]
    pub fn reply(&self, reply: &MsgBuf, msg: &'static tcu::Message) -> Result<(), Error> {
        let off = tcu::TCU::msg_to_offset(self.address().unwrap(), msg);
        tcu::TCU::reply(self.ep().unwrap(), reply, off)
    }

    /// Marks the given message as 'read', allowing the TCU to overwrite it with a new message.
    #[inline(always)]
    pub fn ack_msg(&self, msg: &tcu::Message) -> Result<(), Error> {
        let off = tcu::TCU::msg_to_offset(self.address().unwrap(), msg);
        tcu::TCU::ack_msg(self.ep().unwrap(), off)
    }

    /// Waits until a message arrives and returns a reference to the message. If not `None`, the
    /// argument `sgate` denotes the [`SendGate`] that was used to send the request to the
    /// communication for which this method should receive the reply now. If the endpoint associated
    /// with `sgate` becomes invalid, the method stops waiting for a reply assuming that the
    /// communication partner is no longer interested in the communication.
    #[inline(always)]
    pub fn receive(&self, sgate: Option<&SendGate>) -> Result<&'static tcu::Message, Error> {
        let rep = self.ep().unwrap();
        // if the tile is shared with someone else that wants to run, poll a couple of times to
        // prevent too frequent/unnecessary switches.
        let polling = if arch::env::get().shared() { 200 } else { 1 };
        loop {
            for _ in 0..polling {
                let msg_off = tcu::TCU::fetch_msg(rep);
                if let Some(off) = msg_off {
                    let msg = tcu::TCU::offset_to_msg(self.address().unwrap(), off);
                    return Ok(msg);
                }
            }

            if let Some(sg) = sgate {
                if !tcu::TCU::is_valid(sg.ep().unwrap().id()) {
                    return Err(Error::new(Code::NoSEP));
                }
            }

            Activity::own().wait_for(Some(rep), None, None)?;
        }
    }

    /// Drops all messages with given label. That is, these messages will be marked as read.
    pub fn drop_msgs_with(&self, label: tcu::Label) {
        tcu::TCU::drop_msgs_with(self.address().unwrap(), self.ep().unwrap(), label);
    }
}

pub(crate) fn pre_init() {
    let eps_start = arch::env::get().first_std_ep();
    let mut rbuf = arch::env::get().tile_desc().rbuf_std_space().0;
    // safety: we only do that during (re-)initialization so that there are no outstanding
    // references to the old value. note that we need to use reset() here, because we also use it
    // for activity::run() to reinitialize everything.
    unsafe {
        SYS_RGATE.reset(RecvGate::new_def(
            INVALID_SEL,
            eps_start + tcu::SYSC_REP_OFF,
            rbuf,
            math::next_log2(cfg::SYSC_RBUF_SIZE),
        ));
        rbuf += cfg::SYSC_RBUF_SIZE;

        UPC_RGATE.reset(RecvGate::new_def(
            INVALID_SEL,
            eps_start + tcu::UPCALL_REP_OFF,
            rbuf,
            math::next_log2(cfg::UPCALL_RBUF_SIZE),
        ));
        rbuf += cfg::UPCALL_RBUF_SIZE;

        DEF_RGATE.reset(RecvGate::new_def(
            INVALID_SEL,
            eps_start + tcu::DEF_REP_OFF,
            rbuf,
            math::next_log2(cfg::DEF_RBUF_SIZE),
        ));
    }
}

impl ops::Drop for RecvGate {
    fn drop(&mut self) {
        self.deactivate();
        if let Some(b) = self.buf.take() {
            free_rbuf(b);
        }
    }
}
