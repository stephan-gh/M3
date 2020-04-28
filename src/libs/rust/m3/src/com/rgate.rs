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
use cap::{CapFlags, Selector};
use cell::LazyStaticCell;
use cfg;
use com::rbufs::{alloc_rbuf, free_rbuf};
use com::{gate::Gate, GateIStream, RecvBuf, SendGate};
use core::fmt;
use core::ops;
use errors::Error;
use kif::INVALID_SEL;
use math;
use pes::VPE;
use syscalls;
use tcu;
use util;

const DEF_MSG_ORD: u32 = 6;

static SYS_RGATE: LazyStaticCell<RecvGate> = LazyStaticCell::default();
static UPC_RGATE: LazyStaticCell<RecvGate> = LazyStaticCell::default();
static DEF_RGATE: LazyStaticCell<RecvGate> = LazyStaticCell::default();

/// A receive gate (`RecvGate`) can receive messages via TCU from connected [`SendGate`]s and can
/// reply on the received messages.
pub struct RecvGate {
    gate: Gate,
    buf: Option<RecvBuf>,
    buf_addr: Option<usize>,
    order: u32,
    msg_order: u32,
    // TODO this is a workaround for a code-generation bug for arm, which generates
    // "ldm r8!,{r2,r4,r6,r8}" with the EP id loaded into r8 and afterwards increased by 16 because
    // of the "!".
    _dummy: u64,
}

impl fmt::Debug for RecvGate {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(
            f,
            "RecvGate[sel: {}, buf: {:?}, size: {:#0x}, ep: {:?}]",
            self.sel(),
            self.buf,
            1 << self.order,
            self.gate.ep()
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
    /// [`VPE::alloc_sel`] will be used.
    pub fn sel(mut self, sel: Selector) -> Self {
        self.sel = sel;
        self
    }
}

impl RecvGate {
    /// Returns the receive gate to receive system call replies
    pub fn syscall() -> &'static mut RecvGate {
        SYS_RGATE.get_mut()
    }

    /// Returns the receive gate to receive upcalls from the kernel
    pub fn upcall() -> &'static mut RecvGate {
        UPC_RGATE.get_mut()
    }

    /// Returns the default receive gate
    pub fn def() -> &'static mut RecvGate {
        DEF_RGATE.get_mut()
    }

    const fn new_def(sel: Selector, ep: tcu::EpId, addr: usize, order: u32) -> Self {
        RecvGate {
            gate: Gate::new_with_ep(sel, CapFlags::KEEP_CAP, ep),
            buf: None,
            buf_addr: Some(addr),
            order,
            msg_order: order,
            _dummy: 0,
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
            VPE::cur().alloc_sel()
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
            _dummy: 0,
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
            _dummy: 0,
        }
    }

    /// Returns the selector of the gate
    pub fn sel(&self) -> Selector {
        self.gate.sel()
    }

    /// Returns the endpoint of the gate. If the gate is not activated, `None` is returned.
    pub(crate) fn ep(&self) -> Option<tcu::EpId> {
        self.gate.ep().map(|ep| ep.id())
    }

    /// Returns the size of the receive buffer in bytes
    pub fn size(&self) -> usize {
        1 << self.order
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
    pub fn activate_with(&mut self, mem: Selector, off: usize, addr: usize) -> Result<(), Error> {
        let replies = 1 << (self.order - self.msg_order);
        self.gate.activate_rgate(mem, off, replies).map(|_| {
            self.buf_addr = Some(addr);
        })
    }

    /// Deactivates this gate.
    pub fn deactivate(&mut self) {
        self.gate.release(true);
    }

    /// Tries to fetch a message from the receive gate. If there is an unread message, it returns
    /// a [`GateIStream`] for the message. Otherwise it returns None.
    pub fn fetch(&self) -> Option<GateIStream> {
        let msg = tcu::TCUIf::fetch_msg(self);
        if let Some(m) = msg {
            Some(GateIStream::new(m, self))
        }
        else {
            None
        }
    }

    /// Sends `reply` as a reply to the message `msg`.
    #[inline(always)]
    pub fn reply<T>(&self, reply: &[T], msg: &'static tcu::Message) -> Result<(), Error> {
        self.reply_bytes(
            reply.as_ptr() as *const u8,
            reply.len() * util::size_of::<T>(),
            msg,
        )
    }

    /// Sends `reply` with `size` bytes as a reply to the message `msg`.
    #[inline(always)]
    pub fn reply_bytes(
        &self,
        reply: *const u8,
        size: usize,
        msg: &'static tcu::Message,
    ) -> Result<(), Error> {
        tcu::TCUIf::reply(self, reply, size, msg)
    }

    /// Marks the given message as 'read', allowing the TCU to overwrite it with a new message.
    #[inline(always)]
    pub fn ack_msg(&self, msg: &tcu::Message) {
        tcu::TCUIf::ack_msg(self, msg);
    }

    /// Waits until a message arrives and returns a [`GateIStream`] for the message. If not `None`,
    /// the argument `sgate` denotes the [`SendGate`] that was used to send the request to the
    /// communication for which this method should receive the reply now. If the endpoint associated
    /// with `sgate` becomes invalid, the method stops waiting for a reply assuming that the
    /// communication partner is no longer interested in the communication.
    #[inline(always)]
    pub fn receive(&self, sgate: Option<&SendGate>) -> Result<GateIStream, Error> {
        tcu::TCUIf::receive(self, sgate).map(|m| GateIStream::new(m, self))
    }

    /// Drops all messages with given label. That is, these messages will be marked as read.
    pub fn drop_msgs_with(&self, label: tcu::Label) {
        tcu::TCU::drop_msgs_with(self.address().unwrap(), self.ep().unwrap(), label);
    }
}

pub(crate) fn pre_init() {
    let eps_start = arch::env::get().first_std_ep();
    let mut rbuf = arch::env::get().pe_desc().rbuf_std_space().0;
    SYS_RGATE.set(RecvGate::new_def(
        INVALID_SEL,
        eps_start + tcu::SYSC_REP_OFF,
        rbuf,
        math::next_log2(cfg::SYSC_RBUF_SIZE),
    ));
    rbuf += cfg::SYSC_RBUF_SIZE;

    UPC_RGATE.set(RecvGate::new_def(
        INVALID_SEL,
        eps_start + tcu::UPCALL_REP_OFF,
        rbuf,
        math::next_log2(cfg::UPCALL_RBUF_SIZE),
    ));
    rbuf += cfg::UPCALL_RBUF_SIZE;

    DEF_RGATE.set(RecvGate::new_def(
        INVALID_SEL,
        eps_start + tcu::DEF_REP_OFF,
        rbuf,
        math::next_log2(cfg::DEF_RBUF_SIZE),
    ));
}

impl ops::Drop for RecvGate {
    fn drop(&mut self) {
        self.deactivate();
        if let Some(b) = self.buf.take() {
            free_rbuf(b);
        }
    }
}
