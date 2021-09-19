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

use base::serialize::{Marshallable, Unmarshallable};

use crate::cap::Selector;
use crate::cfg;
use crate::col::String;
use crate::com::{GateIStream, RecvGate, SendGate};
use crate::errors::Error;
use crate::goff;
use crate::int_enum;
use crate::kif;
use crate::pes::VPE;
use crate::tcu::PEId;

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

        const USE_RGATE     = 0xA;
        const USE_SGATE     = 0xB;

        const USE_SEM       = 0xC;

        const GET_SERIAL    = 0xD;

        const GET_INFO      = 0xE;
    }
}

#[derive(Debug)]
pub struct ResMngVPEInfo {
    pub id: u32,
    pub layer: u32,
    pub name: String,
    pub daemon: bool,
    pub total_mem: goff,
    pub avail_mem: goff,
    pub pe: PEId,
}

pub enum ResMngVPEInfoResult {
    Info(ResMngVPEInfo),
    Count((usize, u32)),
}

impl Marshallable for ResMngVPEInfoResult {
    fn marshall(&self, s: &mut base::serialize::Sink) {
        match self {
            ResMngVPEInfoResult::Info(i) => {
                s.push(&0);
                s.push(&i.id);
                s.push(&i.layer);
                s.push(&i.name);
                s.push(&i.daemon);
                s.push(&i.total_mem);
                s.push(&i.avail_mem);
                s.push(&i.pe);
            },
            ResMngVPEInfoResult::Count((num, layer)) => {
                s.push(&1);
                s.push(num);
                s.push(layer);
            },
        }
    }
}

impl Unmarshallable for ResMngVPEInfoResult {
    fn unmarshall(s: &mut base::serialize::Source) -> Result<Self, Error> {
        let ty = s.pop::<u64>()?;
        match ty {
            0 => Ok(Self::Info(ResMngVPEInfo {
                id: s.pop()?,
                layer: s.pop()?,
                name: s.pop()?,
                daemon: s.pop()?,
                total_mem: s.pop()?,
                avail_mem: s.pop()?,
                pe: s.pop()?,
            })),
            _ => Ok(Self::Count((s.pop()?, s.pop()?))),
        }
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

    /// Registers a service with given name at selector `dst`, using `sgate` for session creations.
    pub fn reg_service(
        &self,
        dst: Selector,
        sgate: Selector,
        name: &str,
        sessions: usize,
    ) -> Result<(), Error> {
        send_recv_res!(
            &self.sgate,
            RecvGate::def(),
            ResMngOperation::REG_SERV,
            dst,
            sgate,
            sessions,
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
    pub fn alloc_pe(&self, sel: Selector, desc: kif::PEDesc) -> Result<(PEId, kif::PEDesc), Error> {
        let mut reply = send_recv_res!(
            &self.sgate,
            RecvGate::def(),
            ResMngOperation::ALLOC_PE,
            sel,
            desc.value()
        )?;
        let pe_id: PEId = reply.pop()?;
        let raw: kif::PEDescRaw = reply.pop()?;
        Ok((pe_id, kif::PEDesc::new_from(raw)))
    }

    /// Free's the processing element with given selector
    pub fn free_pe(&self, sel: Selector) -> Result<(), Error> {
        send_recv_res!(&self.sgate, RecvGate::def(), ResMngOperation::FREE_PE, sel).map(|_| ())
    }

    /// Attaches to the RecvGate with given name using selector `sel`.
    pub fn use_rgate(&self, sel: Selector, name: &str) -> Result<(u32, u32), Error> {
        let mut reply = self.use_op(ResMngOperation::USE_RGATE, sel, name)?;
        let order = reply.pop()?;
        let msg_order = reply.pop()?;
        Ok((order, msg_order))
    }

    /// Attaches to the SendGate with given name using selector `sel`.
    pub fn use_sgate(&self, sel: Selector, name: &str) -> Result<(), Error> {
        self.use_op(ResMngOperation::USE_SGATE, sel, name)
            .map(|_| ())
    }

    /// Attaches to the semaphore with given name using selector `sel`.
    pub fn use_sem(&self, sel: Selector, name: &str) -> Result<(), Error> {
        self.use_op(ResMngOperation::USE_SEM, sel, name).map(|_| ())
    }

    /// Retrieves the receive gate to receive serial input
    pub fn get_serial(&self, sel: Selector) -> Result<RecvGate, Error> {
        send_recv_res!(
            &self.sgate,
            RecvGate::def(),
            ResMngOperation::GET_SERIAL,
            sel
        )
        .map(|_| RecvGate::new_bind(sel, cfg::SERIAL_BUF_ORD, cfg::SERIAL_BUF_ORD))
    }

    /// Gets the number of available VPEs for `get_vpe_info` and the starting layer.
    pub fn get_vpe_count(&self) -> Result<(usize, u32), Error> {
        match self.vpe_info(None) {
            Ok(ResMngVPEInfoResult::Count((num, layer))) => Ok((num, layer)),
            Err(e) => Err(e),
            _ => panic!("unexpected info type"),
        }
    }

    /// Retrieves information about the VPE with given index.
    pub fn get_vpe_info(&self, vpe_idx: usize) -> Result<ResMngVPEInfo, Error> {
        match self.vpe_info(Some(vpe_idx)) {
            Ok(ResMngVPEInfoResult::Info(i)) => Ok(i),
            Err(e) => Err(e),
            _ => panic!("unexpected info type"),
        }
    }

    fn vpe_info(&self, vpe_idx: Option<usize>) -> Result<ResMngVPEInfoResult, Error> {
        send_recv_res!(
            &self.sgate,
            RecvGate::def(),
            ResMngOperation::GET_INFO,
            vpe_idx.unwrap_or(usize::MAX)
        )
        .and_then(|mut is| is.pop())
    }

    fn use_op(&self, op: ResMngOperation, sel: Selector, name: &str) -> Result<GateIStream, Error> {
        send_recv_res!(&self.sgate, RecvGate::def(), op, sel, name)
    }
}

impl Drop for ResMng {
    fn drop(&mut self) {
        if self.vpe_sel != kif::INVALID_SEL {
            send_recv_res!(
                &VPE::cur().resmng().unwrap().sgate,
                RecvGate::def(),
                ResMngOperation::REM_CHILD,
                self.vpe_sel
            )
            .ok();
        }
    }
}
