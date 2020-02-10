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

use cap;
use com::{RGateArgs, RecvGate, SendGate};
use core::fmt;
use errors::Error;
use goff;
use kif;
use pes::VPE;
use session::ClientSession;

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
    pub struct PagerDelOp : u32 {
        const DATASPACE = 0x0;
        const MEMGATE   = 0x1;
    }
}

int_enum! {
    pub struct PagerOp : u32 {
        const PAGEFAULT = 0x0;
        const CLONE     = 0x1;
        const MAP_ANON  = 0x2;
        const UNMAP     = 0x3;
        const CLOSE     = 0x4;
    }
}

bitflags! {
    pub struct MapFlags : u32 {
        const PRIVATE = 0x0;
        const SHARED  = 0x2000;
        const UNINIT  = 0x4000;
        const NOLPAGE = 0x8000;
    }
}

impl Pager {
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
        let sgate = SendGate::new_bind(sess.obtain_obj()?);
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
        let mut args = kif::syscalls::ExchangeArgs::default();
        // dummy arg to distinguish from the get_sgate operation
        args.push_ival(0);
        let res = self.sess.obtain(1, &mut args)?;
        let sess = ClientSession::new_bind(res.start());

        // get send gates for us and our child
        let parent_sgate = SendGate::new_bind(sess.obtain_obj()?);
        let child_sgate = SendGate::new_bind(sess.obtain_obj()?);

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

    /// Delegates the required capabilities from `vpe` to the server.
    pub fn delegate_caps(&mut self, vpe: &VPE) -> Result<(), Error> {
        // we only need to do that for clones
        if self.close {
            const_assert!(kif::SEL_VPE + 1 == kif::SEL_MEM);
            let crd = kif::CapRngDesc::new(kif::CapType::OBJECT, vpe.sel(), 2);
            self.sess.delegate_crd(crd)
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
        Ok(reply.pop())
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
        let mut args = kif::syscalls::ExchangeArgs::new(6, kif::syscalls::ExchangeUnion {
            i: [
                u64::from(PagerDelOp::DATASPACE.val),
                addr as u64,
                len as u64,
                u64::from(prot.bits()),
                u64::from(flags.bits()),
                off as u64,
                0,
                0,
            ],
        });

        let crd = kif::CapRngDesc::new(kif::CapType::OBJECT, sess.sel(), 1);
        self.sess.delegate(crd, &mut args)?;
        Ok(args.ival(0) as goff)
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
