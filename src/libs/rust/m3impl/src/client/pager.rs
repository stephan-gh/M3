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

use bitflags::bitflags;
use core::fmt;

use crate::cap;
use crate::client::ClientSession;
use crate::com::{opcodes, RGateArgs, RecvCap, RecvGate, SendGate};
use crate::errors::Error;
use crate::kif;
use crate::mem::VirtAddr;
use crate::serialize::{Deserialize, Serialize};
use crate::syscalls;
use crate::tiles::ChildActivity;

/// Represents a session at the pager
///
/// The pager allows to map memory into the virtual address space and is responsible to resolve page
/// faults when this memory is accessed.
pub struct Pager {
    sess: ClientSession,
    req_sgate: Option<SendGate>,
    child_sgate: cap::Selector,
    pf_rgate: Option<RecvCap>,
    pf_sgate: cap::Selector,
}

bitflags! {
    /// The mapping flags
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(crate = "base::serde")]
    pub struct MapFlags : u32 {
        /// A private mapping, not shared with anyone else
        const PRIVATE = 0x0;
        /// A shared mapping
        const SHARED  = 0x2000;
        /// Do not initialize the memory
        const UNINIT  = 0x4000;
        /// Do not use a large page, even if possible
        const NOLPAGE = 0x8000;
    }
}

impl Pager {
    /// Creates a new session with given gates (for the pager).
    pub fn new(
        sess: ClientSession,
        pf_sgate: cap::Selector,
        child_sgate: cap::Selector,
    ) -> Result<Self, Error> {
        let pf_rgate = Some(RecvCap::new_with(
            RGateArgs::default().order(6).msg_order(6),
        )?);

        Ok(Pager {
            sess,
            req_sgate: None,
            child_sgate,
            pf_rgate,
            pf_sgate,
        })
    }

    /// Binds a new pager-session to given selector (for childs).
    pub(crate) fn new_bind(sess_sel: cap::Selector, sgate_sel: cap::Selector) -> Self {
        let sess = ClientSession::new_bind(sess_sel);
        let sgate = SendGate::new_bind(sgate_sel).unwrap();
        Pager {
            sess,
            req_sgate: Some(sgate),
            child_sgate: kif::INVALID_SEL,
            pf_rgate: None,
            pf_sgate: kif::INVALID_SEL,
        }
    }

    /// Clones the session to be shared with the given activity.
    pub(crate) fn new_clone(&self) -> Result<Self, Error> {
        let res = self
            .sess
            .obtain(1, |os| os.push(opcodes::Pager::AddChild), |_| Ok(()))?;
        let sess = ClientSession::new_owned_bind(res.start());

        // get send gates for us and our child
        let child_sgate = sess.connect()?.sel();
        let pf_sgate = sess.connect()?.sel();
        let req_sgate = Some(sess.connect()?);

        let pf_rgate = Some(RecvCap::new_with(
            RGateArgs::default().order(6).msg_order(6),
        )?);
        Ok(Pager {
            sess,
            child_sgate,
            req_sgate,
            pf_rgate,
            pf_sgate,
        })
    }

    /// Initializes this pager session by delegating the activity cap to the server.
    pub(crate) fn init(&mut self, act: &ChildActivity) -> Result<(), Error> {
        // activate send and receive gate for page faults
        syscalls::activate(act.sel() + 1, self.pf_sgate, kif::INVALID_SEL, 0)?;
        syscalls::activate(
            act.sel() + 2,
            self.pf_rgate.as_ref().unwrap().sel(),
            kif::INVALID_SEL,
            0,
        )?;

        // delegate session and sgate caps to child
        act.delegate_obj(self.sel())?;
        act.delegate_obj(self.sgate_sel())?;

        // we only need to do that for clones
        if self.sess.is_owned() {
            let crd = kif::CapRngDesc::new(kif::CapType::Object, act.sel(), 1);
            self.sess
                .delegate(crd, |os| os.push(opcodes::Pager::Init), |_| Ok(()))
        }
        else {
            Ok(())
        }
    }

    /// Returns the sessions capability selector.
    pub fn sel(&self) -> cap::Selector {
        self.sess.sel()
    }

    /// Returns the send gate capability selector for the child.
    pub(crate) fn sgate_sel(&self) -> cap::Selector {
        self.child_sgate
    }

    /// Performs the clone-operation on server-side using copy-on-write.
    #[allow(clippy::should_implement_trait)]
    pub fn clone(&self) -> Result<(), Error> {
        send_recv_res!(
            self.req_sgate.as_ref().unwrap(),
            RecvGate::def(),
            opcodes::Pager::Clone
        )
        .map(|_| ())
    }

    /// Sends a page fault for the virtual address `virt` for given access type to the server.
    ///
    /// This method just exists for completeness, because page faults are send by TileMux to the
    /// pager, which uses a lower-level API. However, this method can still be used to let the pager
    /// resolve a page fault for a particular address.
    pub fn pagefault(&self, virt: VirtAddr, access: kif::Perm) -> Result<(), Error> {
        send_recv_res!(
            self.req_sgate.as_ref().unwrap(),
            RecvGate::def(),
            opcodes::Pager::Pagefault,
            virt,
            access.bits()
        )
        .map(|_| ())
    }

    /// Maps `len` bytes of anonymous memory to virtual address `virt` with permissions `prot`.
    pub fn map_anon(
        &self,
        virt: VirtAddr,
        len: usize,
        prot: kif::Perm,
        flags: MapFlags,
    ) -> Result<VirtAddr, Error> {
        let mut reply = send_recv_res!(
            self.req_sgate.as_ref().unwrap(),
            RecvGate::def(),
            opcodes::Pager::MapAnon,
            virt,
            len,
            prot.bits(),
            flags.bits()
        )?;
        reply.pop()
    }

    /// Maps a dataspace of `len` bytes handled by given session to virtual address `virt` with
    /// permissions `prot`.
    pub fn map_ds(
        &self,
        virt: VirtAddr,
        len: usize,
        off: usize,
        prot: kif::Perm,
        flags: MapFlags,
        sess: &ClientSession,
    ) -> Result<VirtAddr, Error> {
        let crd = kif::CapRngDesc::new(kif::CapType::Object, sess.sel(), 1);
        let mut res = VirtAddr::default();
        self.sess.delegate(
            crd,
            |os| {
                os.push(opcodes::Pager::MapDS);
                os.push(virt);
                os.push(len);
                os.push(prot);
                os.push(flags);
                os.push(off);
            },
            |is| {
                res = is.pop()?;
                Ok(())
            },
        )?;

        Ok(res)
    }

    /// Maps `len` bytes of the given memory (`mem`) at the virtual address `virt` with permissions
    /// `prot`.
    ///
    /// As `mem` already refers to allocated physical memory, no page faults will occur.
    pub fn map_mem(
        &self,
        virt: VirtAddr,
        mem: cap::Selector,
        len: usize,
        prot: kif::Perm,
    ) -> Result<VirtAddr, Error> {
        let crd = kif::CapRngDesc::new(kif::CapType::Object, mem, 1);
        let mut res = VirtAddr::default();
        self.sess.delegate(
            crd,
            |os| {
                os.push(opcodes::Pager::MapMem);
                os.push(virt);
                os.push(len);
                os.push(prot);
            },
            |is| {
                res = is.pop()?;
                Ok(())
            },
        )?;

        Ok(res)
    }

    /// Unaps the mapping at virtual address `virt`.
    pub fn unmap(&self, virt: VirtAddr) -> Result<(), Error> {
        send_recv_res!(
            self.req_sgate.as_ref().unwrap(),
            RecvGate::def(),
            opcodes::Pager::Unmap,
            virt
        )
        .map(|_| ())
    }
}

impl fmt::Debug for Pager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "Pager[sel: {}]", self.sel(),)
    }
}
