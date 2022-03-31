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
use crate::errors::Error;
use crate::kif;
use crate::serialize::{Sink, Source};
use crate::syscalls;
use crate::tiles::Activity;

/// Represents an established connection to a server that can be used to exchange capabilities.
pub struct ClientSession {
    cap: Capability,
    close: bool,
}

impl ClientSession {
    /// Creates a new `ClientSession` by connecting to the service with given name.
    pub fn new(name: &str) -> Result<Self, Error> {
        Self::new_with_sel(name, Activity::own().alloc_sel())
    }

    /// Creates a new `ClientSession` by connecting to the service with given name, using the given
    /// capability selector for the session.
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

    /// Returns the capability selector.
    pub fn sel(&self) -> Selector {
        self.cap.sel()
    }

    /// Delegates the object capability with selector `sel` to the server.
    pub fn delegate_obj(&self, sel: Selector) -> Result<(), Error> {
        let crd = kif::CapRngDesc::new(kif::CapType::OBJECT, sel, 1);
        self.delegate_crd(crd)
    }

    /// Delegates the given capability range to the server.
    pub fn delegate_crd(&self, crd: kif::CapRngDesc) -> Result<(), Error> {
        self.delegate(crd, |_| {}, |_| Ok(()))
    }

    /// Delegates the given capability range to the server, using `pre` and `post` for input and
    /// output arguments. `pre` is called with a [`Sink`] before the delegation operation, allowing
    /// to pass arguments to the server. `post` is called with a [`Source`] after the delegation
    /// operation, allowing to get arguments from the server.
    pub fn delegate<PRE, POST>(
        &self,
        crd: kif::CapRngDesc,
        pre: PRE,
        post: POST,
    ) -> Result<(), Error>
    where
        PRE: Fn(&mut Sink<'_>),
        POST: FnMut(&mut Source<'_>) -> Result<(), Error>,
    {
        self.delegate_for(Activity::own().sel(), crd, pre, post)
    }

    /// Delegates the given capability range from `act` to the server, using `pre` and `post` for
    /// input and output arguments. `pre` is called with a [`Sink`] before the delegation operation,
    /// allowing to pass arguments to the server. `post` is called with a [`Source`] after the
    /// delegation operation, allowing to get arguments from the server.
    pub fn delegate_for<PRE, POST>(
        &self,
        act: Selector,
        crd: kif::CapRngDesc,
        pre: PRE,
        post: POST,
    ) -> Result<(), Error>
    where
        PRE: Fn(&mut Sink<'_>),
        POST: FnMut(&mut Source<'_>) -> Result<(), Error>,
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
    /// using `pre` and `post` for input and output arguments. `pre` is called with a [`Sink`]
    /// before the obtain operation, allowing to pass arguments to the server. `post` is called with
    /// a [`Source`] after the obtain operation, allowing to get arguments from the server.
    pub fn obtain<PRE, POST>(
        &self,
        count: u64,
        pre: PRE,
        post: POST,
    ) -> Result<kif::CapRngDesc, Error>
    where
        PRE: Fn(&mut Sink<'_>),
        POST: FnMut(&mut Source<'_>) -> Result<(), Error>,
    {
        let caps = Activity::own().alloc_sels(count);
        let crd = kif::CapRngDesc::new(kif::CapType::OBJECT, caps, count);
        self.obtain_for(Activity::own().sel(), crd, pre, post)?;
        Ok(crd)
    }

    /// Obtains `count` capabilities from the server for activity `act`, using `pre` and `post` for input
    /// and output arguments. `pre` is called with a [`Sink`] before the obtain operation, allowing
    /// to pass arguments to the server. `post` is called with a [`Source`] after the obtain
    /// operation, allowing to get arguments from the server.
    pub fn obtain_for<PRE, POST>(
        &self,
        act: Selector,
        crd: kif::CapRngDesc,
        pre: PRE,
        post: POST,
    ) -> Result<(), Error>
    where
        PRE: Fn(&mut Sink<'_>),
        POST: FnMut(&mut Source<'_>) -> Result<(), Error>,
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
