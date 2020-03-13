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

use cap::Selector;
use com::{RecvGate, SendGate};
use errors::Error;
use goff;
use kif;
use pes::VPE;

int_enum! {
    /// The resource manager calls
    pub struct ResMngOperation : u64 {
        const REG_SERV      = 0x0;
        const UNREG_SERV    = 0x1;

        const OPEN_SESS     = 0x2;
        const CLOSE_SESS    = 0x3;

        const ADD_CHILD     = 0x4;
        const REM_CHILD     = 0x5;

        const ALLOC_MEM     = 0x6;
        const FREE_MEM      = 0x7;

        const ALLOC_PE      = 0x8;
        const FREE_PE       = 0x9;

        const USE_SEM       = 0xA;
    }
}

/// Represents a connection to the resource manager.
///
/// The resource manager is used to request access to resources like memory and services and is
/// provided by any of the parent VPEs.
pub struct ResMng {
    sgate: SendGate,
    vpe_sel: Selector,
}

impl ResMng {
    /// Creates a new `ResMng` with given [`SendGate`] to send requests to the server.
    pub fn new(sgate: SendGate) -> Self {
        ResMng {
            sgate,
            vpe_sel: kif::INVALID_SEL,
        }
    }

    /// Returns the capability selector of the [`SendGate`] used for requests.
    pub fn sel(&self) -> Selector {
        self.sgate.sel()
    }

    /// Clones this connection to be used by the given VPE as well. `name` specifies the name of the
    /// VPE.
    pub fn clone(&self, vpe: &mut VPE, name: &str) -> Result<Self, Error> {
        let sgate_sel = vpe.alloc_sel();
        send_recv_res!(
            &self.sgate,
            RecvGate::def(),
            ResMngOperation::ADD_CHILD,
            vpe.sel(),
            sgate_sel,
            name
        )?;

        Ok(ResMng {
            sgate: SendGate::new_bind(sgate_sel),
            vpe_sel: vpe.sel(),
        })
    }

    /// Registers a service with given name at selector `dst`, using `rgate` to receive service
    /// calls.
    pub fn reg_service(&self, dst: Selector, rgate: Selector, name: &str) -> Result<(), Error> {
        self.reg_child_service(0, dst, rgate, name)
    }

    /// Registers a service for the child VPE `child` with given name at selector `dst`, using
    /// `rgate` to receive service calls.
    pub fn reg_child_service(
        &self,
        child: Selector,
        dst: Selector,
        rgate: Selector,
        name: &str,
    ) -> Result<(), Error> {
        send_recv_res!(
            &self.sgate,
            RecvGate::def(),
            ResMngOperation::REG_SERV,
            child,
            dst,
            rgate,
            name
        )
        .map(|_| ())
    }

    /// Unregisters the service with given selector. If `notify` is true, the server will be
    /// notified via the `SHUTDOWN` service call.
    pub fn unreg_service(&self, sel: Selector, notify: bool) -> Result<(), Error> {
        send_recv_res!(
            &self.sgate,
            RecvGate::def(),
            ResMngOperation::UNREG_SERV,
            sel,
            notify
        )
        .map(|_| ())
    }

    /// Opens a session at service `name` using selector `dst`.
    pub fn open_sess(&self, dst: Selector, name: &str) -> Result<(), Error> {
        send_recv_res!(
            &self.sgate,
            RecvGate::def(),
            ResMngOperation::OPEN_SESS,
            dst,
            name
        )
        .map(|_| ())
    }

    /// Closes the session with given selector.
    pub fn close_sess(&self, sel: Selector) -> Result<(), Error> {
        send_recv_res!(
            &self.sgate,
            RecvGate::def(),
            ResMngOperation::CLOSE_SESS,
            sel
        )
        .map(|_| ())
    }

    /// Allocates `size` bytes of physical memory with given permissions. If `addr` is not `!0`, it
    /// will be allocated at that address.
    pub fn alloc_mem(
        &self,
        dst: Selector,
        addr: goff,
        size: usize,
        perms: kif::Perm,
    ) -> Result<(), Error> {
        send_recv_res!(
            &self.sgate,
            RecvGate::def(),
            ResMngOperation::ALLOC_MEM,
            dst,
            addr,
            size,
            perms.bits()
        )
        .map(|_| ())
    }

    /// Free's the memory with given selector.
    pub fn free_mem(&self, sel: Selector) -> Result<(), Error> {
        send_recv_res!(&self.sgate, RecvGate::def(), ResMngOperation::FREE_MEM, sel).map(|_| ())
    }

    /// Allocates a new processing element of given type and assigns it to selector `sel`.
    pub fn alloc_pe(&self, sel: Selector, desc: kif::PEDesc) -> Result<kif::PEDesc, Error> {
        let mut reply = send_recv_res!(
            &self.sgate,
            RecvGate::def(),
            ResMngOperation::ALLOC_PE,
            sel,
            desc.value()
        )?;
        let raw: kif::PEDescRaw = reply.pop()?;
        Ok(kif::PEDesc::new_from(raw))
    }

    /// Free's the processing element with given selector
    pub fn free_pe(&self, sel: Selector) -> Result<(), Error> {
        send_recv_res!(&self.sgate, RecvGate::def(), ResMngOperation::FREE_PE, sel).map(|_| ())
    }

    /// Attaches to the semaphore with given name using selector `sel`.
    pub fn use_sem(&self, sel: Selector, name: &str) -> Result<(), Error> {
        send_recv_res!(
            &self.sgate,
            RecvGate::def(),
            ResMngOperation::USE_SEM,
            sel,
            name
        )
        .map(|_| ())
    }
}

impl Drop for ResMng {
    fn drop(&mut self) {
        if self.vpe_sel != kif::INVALID_SEL {
            send_recv_res!(
                &VPE::cur().resmng().sgate,
                RecvGate::def(),
                ResMngOperation::REM_CHILD,
                self.vpe_sel
            )
            .ok();
        }
    }
}
