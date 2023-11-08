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

use crate::cap::{CapFlags, SelSpace, Selector};
use crate::cell::Ref;
use crate::com::ep::EP;
use crate::com::gate::Gate;
use crate::com::RecvGate;
use crate::errors::Error;
use crate::kif::{INVALID_SEL, UNLIM_CREDITS};
use crate::mem::MsgBuf;
use crate::syscalls;
use crate::tcu;
use crate::tiles::Activity;

/// A send gate sends message via TCU
///
/// The interaction of [`SendGate`]s and [`RecvGate`]s including the message-passing concept is
/// explained [`here`](`RecvGate`).
pub struct SendGate {
    gate: Gate,
}

/// The arguments for [`SendGate`] creations.
pub struct SGateArgs {
    rgate_sel: Selector,
    label: tcu::Label,
    credits: u32,
    sel: Selector,
    flags: CapFlags,
}

impl SGateArgs {
    /// Creates a new `SGateArgs` to send messages to `rgate` with default settings.
    pub fn new(rgate: &RecvGate) -> Self {
        SGateArgs {
            rgate_sel: rgate.sel(),
            label: 0,
            credits: UNLIM_CREDITS,
            sel: INVALID_SEL,
            flags: CapFlags::empty(),
        }
    }

    /// Sets the credits to `credits`.
    pub fn credits(mut self, credits: u32) -> Self {
        self.credits = credits;
        self
    }

    /// Sets the label to `label`.
    pub fn label(mut self, label: tcu::Label) -> Self {
        self.label = label;
        self
    }

    /// Sets the capability selector to use for the [`SendGate`]. Otherwise and by default,
    /// [`SelSpace::get().alloc_sel`](crate::cap::SelSpace::alloc_sel) will be used.
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

impl SendGate {
    pub(crate) const fn new_def(sel: Selector, ep: tcu::EpId) -> Self {
        SendGate {
            gate: Gate::new_with_ep(sel, CapFlags::KEEP_CAP, ep),
        }
    }

    /// Creates a new `SendGate` that can send messages to `rgate`.
    pub fn new(rgate: &RecvGate) -> Result<Self, Error> {
        Self::new_with(SGateArgs::new(rgate))
    }

    /// Creates a new `SendGate` with given arguments.
    pub fn new_with(args: SGateArgs) -> Result<Self, Error> {
        let sel = if args.sel == INVALID_SEL {
            SelSpace::get().alloc_sel()
        }
        else {
            args.sel
        };

        syscalls::create_sgate(sel, args.rgate_sel, args.label, args.credits)?;
        Ok(SendGate {
            gate: Gate::new(sel, args.flags),
        })
    }

    /// Creates the `SendGate` with given name as defined in the application's configuration
    pub fn new_named(name: &str) -> Result<Self, Error> {
        let sel = SelSpace::get().alloc_sel();
        Activity::own().resmng().unwrap().use_sgate(sel, name)?;
        Ok(SendGate {
            gate: Gate::new(sel, CapFlags::empty()),
        })
    }

    /// Binds a new `SendGate` to the given capability selector.
    pub fn new_bind(sel: Selector) -> Self {
        SendGate {
            gate: Gate::new(sel, CapFlags::KEEP_CAP),
        }
    }

    /// Returns the capability selector.
    pub fn sel(&self) -> Selector {
        self.gate.sel()
    }

    /// Returns whether the TCU EP has credits to send a message
    pub fn can_send(&self) -> Result<bool, Error> {
        let ep = self.activate()?;
        Ok(tcu::TCU::credits(ep)? > 0)
    }

    /// Returns the number of available credits
    pub fn credits(&self) -> Result<u32, Error> {
        let ep = self.activate()?;
        tcu::TCU::credits(ep)
    }

    /// Returns the endpoint of the gate. If the gate is not activated, `None` is returned.
    pub(crate) fn ep(&self) -> Option<Ref<'_, EP>> {
        self.gate.ep()
    }

    /// Activites this `SendGate` in case it was not already activated.
    ///
    /// Note that the gate is automatically activated when used and does not need to be activated
    /// explicitly.
    ///
    /// Returns the chosen endpoint number.
    pub fn activate(&self) -> Result<tcu::EpId, Error> {
        self.gate.activate()
    }

    /// Deactivates this `SendGate` in case it was already activated
    pub fn deactivate(&mut self) {
        self.gate.release(false);
    }

    /// Sends `msg` to the associated [`RecvGate`] and uses `reply_gate` to receive
    /// a reply.
    #[inline(always)]
    pub fn send(&self, msg: &MsgBuf, reply_gate: &RecvGate) -> Result<(), Error> {
        self.send_with_rlabel(msg, reply_gate, 0)
    }

    /// Sends the message `msg` of `len` bytes via given endpoint. The message address needs to be
    /// 16-byte aligned and `msg`..`msg` + `len` cannot contain a page boundary.
    #[inline(always)]
    pub fn send_aligned(
        &self,
        msg: *const u8,
        len: usize,
        reply_gate: &RecvGate,
    ) -> Result<(), Error> {
        let ep = self.activate()?;
        let rep = reply_gate.ensure_activated()?;
        tcu::TCU::send_aligned(ep, msg, len, 0, rep)
    }

    /// Sends `msg` to the associated [`RecvGate`], uses `reply_gate` to receive the reply, and lets
    /// the communication partner use the label `rlabel` for the reply.
    #[inline(always)]
    pub fn send_with_rlabel(
        &self,
        msg: &MsgBuf,
        reply_gate: &RecvGate,
        rlabel: tcu::Label,
    ) -> Result<(), Error> {
        let ep = self.activate()?;
        let rep = reply_gate.ensure_activated()?;
        tcu::TCU::send(ep, msg, rlabel, rep)
    }

    /// Sends `msg` to the associated [`RecvGate`] and receives the reply from the set reply gate.
    /// Returns the received reply.
    #[inline(always)]
    pub fn call(
        &self,
        msg: &MsgBuf,
        reply_gate: &RecvGate,
    ) -> Result<&'static tcu::Message, Error> {
        let ep = self.activate()?;
        let rep = reply_gate.ensure_activated()?;
        tcu::TCU::send(ep, msg, 0, rep)?;
        reply_gate.receive(Some(self))
    }
}

impl fmt::Debug for SendGate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "SendGate[sel: {}, ep: {:?}]",
            self.sel(),
            self.gate.epid()
        )
    }
}
