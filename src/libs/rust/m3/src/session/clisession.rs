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

use cap::{CapFlags, Capability, Selector};
use com::{SliceSink, SliceSource};
use core::fmt;
use errors::Error;
use kif;
use pes::VPE;
use syscalls;

/// Represents an established connection to a server that can be used to exchange capabilities.
pub struct ClientSession {
    cap: Capability,
    close: bool,
}

impl ClientSession {
    /// Creates a new `ClientSession` by connecting to the service with given name.
    pub fn new(name: &str) -> Result<Self, Error> {
        Self::new_with_sel(name, VPE::cur().alloc_sel())
    }

    /// Creates a new `ClientSession` by connecting to the service with given name, using the given
    /// capability selector for the session.
    pub fn new_with_sel(name: &str, sel: Selector) -> Result<Self, Error> {
        VPE::cur().resmng().open_sess(sel, name)?;

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
    /// output arguments. `pre` is called with a `SliceSink` before the delegation operation,
    /// allowing to pass arguments to the server. `post` is called with a `SliceSource` after the
    /// delegation operation, allowing to get arguments from the server.
    pub fn delegate<PRE, POST>(
        &self,
        crd: kif::CapRngDesc,
        pre: PRE,
        post: POST,
    ) -> Result<(), Error>
    where
        PRE: Fn(&mut SliceSink),
        POST: FnMut(&mut SliceSource) -> Result<(), Error>,
    {
        self.delegate_for(VPE::cur().sel(), crd, pre, post)
    }

    /// Delegates the given capability range from `vpe` to the server, using `pre` and `post` for
    /// input and output arguments. `pre` is called with a `SliceSink` before the delegation
    /// operation, allowing to pass arguments to the server. `post` is called with a `SliceSource`
    /// after the delegation operation, allowing to get arguments from the server.
    pub fn delegate_for<PRE, POST>(
        &self,
        vpe: Selector,
        crd: kif::CapRngDesc,
        pre: PRE,
        post: POST,
    ) -> Result<(), Error>
    where
        PRE: Fn(&mut SliceSink),
        POST: FnMut(&mut SliceSource) -> Result<(), Error>,
    {
        syscalls::delegate(vpe, self.sel(), crd, pre, post)
    }

    /// Obtains an object capability from the server and returns its selector.
    pub fn obtain_obj(&self) -> Result<Selector, Error> {
        self.obtain_crd(1).map(|res| res.start())
    }

    /// Obtains `count` capabilities from the server and returns the capability range descriptor.
    pub fn obtain_crd(&self, count: u32) -> Result<kif::CapRngDesc, Error> {
        self.obtain(count, |_| {}, |_| Ok(()))
    }

    /// Obtains `count` capabilities from the server and returns the capability range descriptor,
    /// using `pre` and `post` for input and output arguments. `pre` is called with a `SliceSink`
    /// before the obtain operation, allowing to pass arguments to the server. `post` is called with
    /// a `SliceSource` after the obtain operation, allowing to get arguments from the server.
    pub fn obtain<PRE, POST>(
        &self,
        count: u32,
        pre: PRE,
        post: POST,
    ) -> Result<kif::CapRngDesc, Error>
    where
        PRE: Fn(&mut SliceSink),
        POST: FnMut(&mut SliceSource) -> Result<(), Error>,
    {
        let caps = VPE::cur().alloc_sels(count);
        let crd = kif::CapRngDesc::new(kif::CapType::OBJECT, caps, count);
        self.obtain_for(VPE::cur().sel(), crd, pre, post)?;
        Ok(crd)
    }

    /// Obtains `count` capabilities from the server for VPE `vpe`, using `pre` and `post` for input
    /// and output arguments. `pre` is called with a `SliceSink` before the obtain operation,
    /// allowing to pass arguments to the server. `post` is called with a `SliceSource` after the
    /// obtain operation, allowing to get arguments from the server.
    pub fn obtain_for<PRE, POST>(
        &self,
        vpe: Selector,
        crd: kif::CapRngDesc,
        pre: PRE,
        post: POST,
    ) -> Result<(), Error>
    where
        PRE: Fn(&mut SliceSink),
        POST: FnMut(&mut SliceSource) -> Result<(), Error>,
    {
        syscalls::obtain(vpe, self.sel(), crd, pre, post)
    }
}

impl Drop for ClientSession {
    fn drop(&mut self) {
        if self.close {
            VPE::cur().resmng().close_sess(self.sel()).ok();
        }
    }
}

impl fmt::Debug for ClientSession {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "ClientSession[sel: {}]", self.sel())
    }
}
