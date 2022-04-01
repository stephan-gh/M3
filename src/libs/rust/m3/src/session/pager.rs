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
use crate::com::{MemGate, RGateArgs, RecvGate, SendGate};
use crate::errors::Error;
use crate::goff;
use crate::int_enum;
use crate::kif;
use crate::session::ClientSession;
use crate::syscalls;
use crate::tiles::ChildActivity;

/// Represents a session at the pager.
///
/// The pager is responsible to resolve page faults and allows to create memory mappings.
pub struct Pager {
    sess: ClientSession,
    req_sgate: SendGate,
    child_sgate: cap::Selector,
    pf_rgate: RecvGate,
    pf_sgate: SendGate,
    close: bool,
}

int_enum! {
    /// The pager's operations
    pub struct PagerOp : u32 {
        /// A page fault
        const PAGEFAULT = 0x0;
        /// Initializes the pager session
        const INIT      = 0x1;
        /// Adds a child activity to the pager session
        const ADD_CHILD = 0x2;
        /// Adds a new send gate to the pager session
        const ADD_SGATE = 0x3;
        /// Clone the address space of a child activity (see `ADD_CHILD`) from the parent
        const CLONE     = 0x4;
        /// Add a new mapping with anonymous memory
        const MAP_ANON  = 0x5;
        /// Add a new data space mapping (e.g., a file)
        const MAP_DS    = 0x6;
        /// Add a new mapping for a given memory capability
        const MAP_MEM   = 0x7;
        /// Remove an existing mapping
        const UNMAP     = 0x8;
        /// Close the pager session
        const CLOSE     = 0x9;
    }
}

bitflags! {
    /// The mapping flags
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
    fn get_sgate(sess: &ClientSession) -> Result<cap::Selector, Error> {
        sess.obtain(
            1,
            |os| os.push_word(u64::from(PagerOp::ADD_SGATE.val)),
            |_| Ok(()),
        )
        .map(|crd| crd.start())
    }

    /// Creates a new session with given `SendGate` (for the pager).
    pub fn new(
        sess: ClientSession,
        pf_sgate: SendGate,
        child_sgate: cap::Selector,
    ) -> Result<Self, Error> {
        let pf_rgate = RecvGate::new_with(RGateArgs::default().order(6).msg_order(6))?;

        Ok(Pager {
            sess,
            req_sgate: SendGate::new_bind(kif::INVALID_SEL),
            child_sgate,
            pf_rgate,
            pf_sgate,
            close: false,
        })
    }

    /// Binds a new pager-session to given selector (for childs).
    #[cfg(not(target_vendor = "host"))]
    pub(crate) fn new_bind(sess_sel: cap::Selector, sgate_sel: cap::Selector) -> Self {
        let sess = ClientSession::new_bind(sess_sel);
        let sgate = SendGate::new_bind(sgate_sel);
        Pager {
            sess,
            req_sgate: sgate,
            child_sgate: kif::INVALID_SEL,
            pf_rgate: RecvGate::new_bind(kif::INVALID_SEL, 6, 6),
            pf_sgate: SendGate::new_bind(kif::INVALID_SEL),
            close: false,
        }
    }

    /// Clones the session to be shared with the given activity.
    pub(crate) fn new_clone(&self) -> Result<Self, Error> {
        let res = self.sess.obtain(
            1,
            |os| os.push_word(u64::from(PagerOp::ADD_CHILD.val)),
            |_| Ok(()),
        )?;
        let sess = ClientSession::new_bind(res.start());

        // get send gates for us and our child
        let child_sgate = Self::get_sgate(&sess)?;
        let req_sgate = SendGate::new_bind(Self::get_sgate(&sess)?);
        let pf_sgate = SendGate::new_bind(Self::get_sgate(&sess)?);

        let pf_rgate = RecvGate::new_with(RGateArgs::default().order(6).msg_order(6))?;
        Ok(Pager {
            sess,
            child_sgate,
            req_sgate,
            pf_rgate,
            pf_sgate,
            close: true,
        })
    }

    /// Initializes this pager session by delegating the activity cap to the server.
    pub(crate) fn init(&mut self, act: &ChildActivity) -> Result<(), Error> {
        // activate send and receive gate for page faults
        syscalls::activate(act.sel() + 1, self.pf_sgate.sel(), kif::INVALID_SEL, 0)?;
        syscalls::activate(act.sel() + 2, self.pf_rgate.sel(), kif::INVALID_SEL, 0)?;

        // delegate session and sgate caps to child
        act.delegate_obj(self.sel())?;
        act.delegate_obj(self.sgate_sel())?;

        // we only need to do that for clones
        if self.close {
            let crd = kif::CapRngDesc::new(kif::CapType::OBJECT, act.sel(), 1);
            self.sess.delegate(
                crd,
                |os| os.push_word(u64::from(PagerOp::INIT.val)),
                |_| Ok(()),
            )
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
        send_recv_res!(&self.req_sgate, RecvGate::def(), PagerOp::CLONE).map(|_| ())
    }

    /// Sends a page fault for the virtual address `addr` for given access type to the server.
    pub fn pagefault(&self, addr: goff, access: u32) -> Result<(), Error> {
        send_recv_res!(
            &self.req_sgate,
            RecvGate::def(),
            PagerOp::PAGEFAULT,
            addr,
            access
        )
        .map(|_| ())
    }

    /// Maps `len` bytes of anonymous memory to virtual address `addr` with permissions `prot`.
    pub fn map_anon(
        &self,
        addr: goff,
        len: usize,
        prot: kif::Perm,
        flags: MapFlags,
    ) -> Result<goff, Error> {
        let mut reply = send_recv_res!(
            &self.req_sgate,
            RecvGate::def(),
            PagerOp::MAP_ANON,
            addr,
            len,
            prot.bits(),
            flags.bits()
        )?;
        reply.pop()
    }

    /// Maps a dataspace of `len` bytes handled by given session to virtual address `addr` with
    /// permissions `prot`.
    pub fn map_ds(
        &self,
        addr: goff,
        len: usize,
        off: usize,
        prot: kif::Perm,
        flags: MapFlags,
        sess: &ClientSession,
    ) -> Result<goff, Error> {
        let crd = kif::CapRngDesc::new(kif::CapType::OBJECT, sess.sel(), 1);
        let mut res = 0;
        self.sess.delegate(
            crd,
            |os| {
                os.push_word(u64::from(PagerOp::MAP_DS.val));
                os.push_word(addr as u64);
                os.push_word(len as u64);
                os.push_word(u64::from(prot.bits()));
                os.push_word(u64::from(flags.bits()));
                os.push_word(off as u64);
            },
            |is| {
                res = is.pop_word()? as goff;
                Ok(())
            },
        )?;

        Ok(res)
    }

    pub fn map_mem(
        &self,
        addr: goff,
        mem: &MemGate,
        len: usize,
        prot: kif::Perm,
    ) -> Result<goff, Error> {
        let crd = kif::CapRngDesc::new(kif::CapType::OBJECT, mem.sel(), 1);
        let mut res = 0;
        self.sess.delegate(
            crd,
            |os| {
                os.push_word(u64::from(PagerOp::MAP_MEM.val));
                os.push_word(addr as u64);
                os.push_word(len as u64);
                os.push_word(u64::from(prot.bits()));
            },
            |is| {
                res = is.pop_word()? as goff;
                Ok(())
            },
        )?;

        Ok(res)
    }

    /// Unaps the mapping at virtual address `addr`.
    pub fn unmap(&self, addr: goff) -> Result<(), Error> {
        send_recv_res!(&self.req_sgate, RecvGate::def(), PagerOp::UNMAP, addr).map(|_| ())
    }
}

impl Drop for Pager {
    fn drop(&mut self) {
        if self.close {
            send_recv_res!(&self.req_sgate, RecvGate::def(), PagerOp::CLOSE).ok();
        }
    }
}

impl fmt::Debug for Pager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "Pager[sel: {}]", self.sel(),)
    }
}
