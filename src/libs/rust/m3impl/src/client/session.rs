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

use crate::cap::{CapFlags, Capability, Selector};
use crate::com::{opcodes, SendGate};
use crate::errors::Error;
use crate::kif;
use crate::serialize::{M3Deserializer, M3Serializer, SliceSink};
use crate::syscalls;
use crate::tiles::Activity;

/// Represents a session at a specific server
///
/// An established session can be used to exchange capabilities and thereby create communication
/// channels, for example.
pub struct ClientSession {
    cap: Capability,
    close: bool,
}

impl ClientSession {
    /// Creates a new `ClientSession` by opening a session at the server with given name.
    pub fn new(name: &str) -> Result<Self, Error> {
        Self::new_with_sel(name, Activity::own().alloc_sel())
    }

    /// Creates a new `ClientSession` by opening a session at the server with given name, using the
    /// given capability selector for the session.
    pub fn new_with_sel(name: &str, sel: Selector) -> Result<Self, Error> {
        Activity::own().resmng().unwrap().open_sess(sel, name)?;

        Ok(ClientSession {
            cap: Capability::new(sel, CapFlags::KEEP_CAP),
            close: true,
        })
    }

    /// Binds a new `ClientSession` to given selector.
    pub fn new_bind(sel: Selector) -> Self {
        ClientSession {
            cap: Capability::new(sel, CapFlags::KEEP_CAP),
            close: false,
        }
    }

    /// Binds a new `ClientSession` to given selector and revokes the cap on drop.
    pub fn new_owned_bind(sel: Selector) -> Self {
        ClientSession {
            cap: Capability::new(sel, CapFlags::empty()),
            close: false,
        }
    }

    /// Return true if this session is owned, i.e., was not created via [`Self::new_bind`] and
    /// therefore will be closed on drop.
    pub fn is_owned(&self) -> bool {
        self.close || !self.cap.flags().contains(CapFlags::KEEP_CAP)
    }

    /// Returns the capability selector.
    pub fn sel(&self) -> Selector {
        self.cap.sel()
    }

    /// Creates a connection for requests to the server
    ///
    /// The method uses the [`Connect`](`opcodes::General::Connect`) operation to obtain a
    /// [`SendGate`] from the server that can be used afterwards to send requests to the server.
    ///
    /// Returns the obtained [`SendGate`]
    pub fn connect(&self) -> Result<SendGate, Error> {
        let sel = Activity::own().alloc_sel();
        self.connect_for(Activity::own(), sel)?;
        Ok(SendGate::new_bind(sel))
    }

    /// Creates a connection for requests to the server for given activity
    ///
    /// The method uses the [`Connect`](`opcodes::General::Connect`) operation to obtain a
    /// [`SendGate`] from the server that can be used afterwards to send requests to the server.
    /// The [`SendGate`] will be obtained for the given activity and bound to the given selector.
    pub fn connect_for(&self, act: &Activity, sel: Selector) -> Result<(), Error> {
        let crd = kif::CapRngDesc::new(kif::CapType::Object, sel, 1);
        self.obtain_for(
            act.sel(),
            crd,
            |is| is.push(opcodes::General::Connect),
            |_| Ok(()),
        )
    }

    /// Delegates the object capability with selector `sel` to the server.
    pub fn delegate_obj(&self, sel: Selector) -> Result<(), Error> {
        let crd = kif::CapRngDesc::new(kif::CapType::Object, sel, 1);
        self.delegate_crd(crd)
    }

    /// Delegates the given capability range to the server.
    pub fn delegate_crd(&self, crd: kif::CapRngDesc) -> Result<(), Error> {
        self.delegate(crd, |_| {}, |_| Ok(()))
    }

    /// Delegates the given capability range to the server, using `pre` and `post` for input and
    /// output arguments. `pre` is called with a [`M3Serializer`] before the delegation operation,
    /// allowing to pass arguments to the server. `post` is called with a [`M3Deserializer`] after
    /// the delegation operation, allowing to get arguments from the server.
    pub fn delegate<PRE, POST>(
        &self,
        crd: kif::CapRngDesc,
        pre: PRE,
        post: POST,
    ) -> Result<(), Error>
    where
        PRE: Fn(&mut M3Serializer<SliceSink<'_>>),
        POST: FnMut(&mut M3Deserializer<'_>) -> Result<(), Error>,
    {
        self.delegate_for(Activity::own().sel(), crd, pre, post)
    }

    /// Delegates the given capability range from `act` to the server, using `pre` and `post` for
    /// input and output arguments. `pre` is called with a [`M3Serializer`] before the delegation
    /// operation, allowing to pass arguments to the server. `post` is called with a
    /// [`M3Deserializer`] after the delegation operation, allowing to get arguments from the
    /// server.
    pub fn delegate_for<PRE, POST>(
        &self,
        act: Selector,
        crd: kif::CapRngDesc,
        pre: PRE,
        post: POST,
    ) -> Result<(), Error>
    where
        PRE: Fn(&mut M3Serializer<SliceSink<'_>>),
        POST: FnMut(&mut M3Deserializer<'_>) -> Result<(), Error>,
    {
        syscalls::delegate(act, self.sel(), crd, pre, post)
    }

    /// Obtains an object capability from the server and returns its selector.
    pub fn obtain_obj(&self) -> Result<Selector, Error> {
        self.obtain_crd(1).map(|res| res.start())
    }

    /// Obtains `count` capabilities from the server and returns the capability range descriptor.
    pub fn obtain_crd(&self, count: u64) -> Result<kif::CapRngDesc, Error> {
        self.obtain(count, |_| {}, |_| Ok(()))
    }

    /// Obtains `count` capabilities from the server and returns the capability range descriptor,
    /// using `pre` and `post` for input and output arguments. `pre` is called with a
    /// [`M3Serializer`] before the obtain operation, allowing to pass arguments to the server.
    /// `post` is called with a [`M3Deserializer`] after the obtain operation, allowing to get
    /// arguments from the server.
    pub fn obtain<PRE, POST>(
        &self,
        count: u64,
        pre: PRE,
        post: POST,
    ) -> Result<kif::CapRngDesc, Error>
    where
        PRE: Fn(&mut M3Serializer<SliceSink<'_>>),
        POST: FnMut(&mut M3Deserializer<'_>) -> Result<(), Error>,
    {
        let caps = Activity::own().alloc_sels(count);
        let crd = kif::CapRngDesc::new(kif::CapType::Object, caps, count);
        self.obtain_for(Activity::own().sel(), crd, pre, post)?;
        Ok(crd)
    }

    /// Obtains `count` capabilities from the server for activity `act`, using `pre` and `post` for
    /// input and output arguments. `pre` is called with a [`M3Serializer`] before the obtain
    /// operation, allowing to pass arguments to the server. `post` is called with a
    /// [`M3Deserializer`] after the obtain operation, allowing to get arguments from the server.
    pub fn obtain_for<PRE, POST>(
        &self,
        act: Selector,
        crd: kif::CapRngDesc,
        pre: PRE,
        post: POST,
    ) -> Result<(), Error>
    where
        PRE: Fn(&mut M3Serializer<SliceSink<'_>>),
        POST: FnMut(&mut M3Deserializer<'_>) -> Result<(), Error>,
    {
        syscalls::obtain(act, self.sel(), crd, pre, post)
    }
}

impl Drop for ClientSession {
    fn drop(&mut self) {
        if self.close {
            Activity::own()
                .resmng()
                .unwrap()
                .close_sess(self.sel())
                .ok();
        }
    }
}

impl fmt::Debug for ClientSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "ClientSession[sel: {}]", self.sel())
    }
}
