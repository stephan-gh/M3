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

use bitflags::bitflags;
use core::fmt;

use crate::cap;
use crate::com::{RGateArgs, RecvGate, SendGate};
use crate::errors::Error;
use crate::goff;
use crate::int_enum;
use crate::kif;
use crate::pes::VPE;
use crate::serialize::{Sink, Source};
use crate::session::ClientSession;

/// Represents a session at the pager.
///
/// The pager is responsible to resolve page faults and allows to create memory mappings.
pub struct Pager {
    sess: ClientSession,
    rgate: RecvGate,
    parent_sgate: SendGate,
    child_sgate: SendGate,
    close: bool,
}

int_enum! {
    /// The pager's operations
    pub struct PagerOp : u32 {
        /// A page fault
        const PAGEFAULT = 0x0;
        /// Initializes the pager session
        const INIT      = 0x1;
        /// Adds a child VPE to the pager session
        const ADD_CHILD = 0x2;
        /// Adds a new send gate to the pager session
        const ADD_SGATE = 0x3;
        /// Clone the address space of a child VPE (see `ADD_CHILD`) from the parent
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
    pub fn new(sess: ClientSession, sgate: SendGate) -> Result<Self, Error> {
        let rgate = RecvGate::new_with(RGateArgs::default().order(6).msg_order(6))?;

        Ok(Pager {
            sess,
            rgate,
            parent_sgate: SendGate::new_bind(kif::INVALID_SEL),
            child_sgate: sgate,
            close: false,
        })
    }

    /// Binds a new pager-session to given selector (for childs).
    pub fn new_bind(sess_sel: cap::Selector) -> Result<Self, Error> {
        let sess = ClientSession::new_bind(sess_sel);
        let sgate = SendGate::new_bind(Self::get_sgate(&sess)?);
        Ok(Pager {
            sess,
            rgate: RecvGate::new_bind(kif::INVALID_SEL, 6, 6),
            parent_sgate: sgate,
            child_sgate: SendGate::new_bind(kif::INVALID_SEL),
            close: false,
        })
    }

    /// Clones the session to be shared with the given VPE.
    pub fn new_clone(&self) -> Result<Self, Error> {
        let res = self.sess.obtain(
            1,
            |os| os.push_word(u64::from(PagerOp::ADD_CHILD.val)),
            |_| Ok(()),
        )?;
        let sess = ClientSession::new_bind(res.start());

        // get send gates for us and our child
        let parent_sgate = SendGate::new_bind(Self::get_sgate(&sess)?);
        let child_sgate = SendGate::new_bind(Self::get_sgate(&sess)?);

        let rgate = RecvGate::new_with(RGateArgs::default().order(6).msg_order(6))?;
        Ok(Pager {
            sess,
            rgate,
            parent_sgate,
            child_sgate,
            close: true,
        })
    }

    /// Returns the sessions capability selector.
    pub fn sel(&self) -> cap::Selector {
        self.sess.sel()
    }

    /// Returns the [`SendGate`] used by the parent to send requests to the pager.
    pub fn parent_sgate(&self) -> &SendGate {
        &self.parent_sgate
    }

    /// Returns the [`SendGate`] used by the child to send page faults to the pager.
    pub fn child_sgate(&self) -> &SendGate {
        &self.child_sgate
    }

    /// Returns the [`RecvGate`] used to receive page fault replies.
    pub fn child_rgate(&self) -> &RecvGate {
        &self.rgate
    }

    /// Initializes this pager session by delegating the VPE cap to the server.
    pub fn init(&mut self, vpe: &VPE) -> Result<(), Error> {
        // we only need to do that for clones
        if self.close {
            let crd = kif::CapRngDesc::new(kif::CapType::OBJECT, vpe.sel(), 1);
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

    /// Performs the clone-operation on server-side using copy-on-write.
    #[allow(clippy::should_implement_trait)]
    pub fn clone(&self) -> Result<(), Error> {
        send_recv_res!(&self.parent_sgate, RecvGate::def(), PagerOp::CLONE).map(|_| ())
    }

    /// Sends a page fault for the virtual address `addr` for given access type to the server.
    pub fn pagefault(&self, addr: goff, access: u32) -> Result<(), Error> {
        send_recv_res!(
            &self.parent_sgate,
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
            &self.parent_sgate,
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

    /// Unaps the mapping at virtual address `addr`.
    pub fn unmap(&self, addr: goff) -> Result<(), Error> {
        send_recv_res!(&self.parent_sgate, RecvGate::def(), PagerOp::UNMAP, addr).map(|_| ())
    }
}

impl Drop for Pager {
    fn drop(&mut self) {
        if self.close {
            send_recv_res!(&self.parent_sgate, RecvGate::def(), PagerOp::CLOSE).ok();
        }
    }
}

impl fmt::Debug for Pager {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "Pager[sel: {}]", self.sel(),)
    }
}
