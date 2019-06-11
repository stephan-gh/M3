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
use vpe::VPE;

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

        const USE_SEM       = 0x8;
    }
}

pub struct ResMng {
    sgate: SendGate,
    vpe_sel: Selector,
}

impl ResMng {
    pub fn new(sgate: SendGate) -> Self {
        ResMng {
            sgate: sgate,
            vpe_sel: kif::INVALID_SEL,
        }
    }

    pub fn sel(&self) -> Selector {
        self.sgate.sel()
    }

    pub fn clone(&self, vpe: &mut VPE, name: &str) -> Result<Self, Error> {
        let sgate_sel = vpe.alloc_sel();
        send_recv_res!(
            &self.sgate, RecvGate::def(),
            ResMngOperation::ADD_CHILD, vpe.sel(), sgate_sel, name
        )?;

        Ok(ResMng {
            sgate: SendGate::new_bind(sgate_sel),
            vpe_sel: vpe.sel()
        })
    }

    pub fn reg_service(&self, child: Selector, dst: Selector,
                       rgate: Selector, name: &str) -> Result<(), Error> {
        send_recv_res!(
            &self.sgate, RecvGate::def(),
            ResMngOperation::REG_SERV, child, dst, rgate, name
        ).map(|_| ())
    }

    pub fn unreg_service(&self, sel: Selector, notify: bool) -> Result<(), Error> {
        send_recv_res!(
            &self.sgate, RecvGate::def(),
            ResMngOperation::UNREG_SERV, sel, notify
        ).map(|_| ())
    }

    pub fn open_sess(&self, dst: Selector, name: &str) -> Result<(), Error> {
        send_recv_res!(
            &self.sgate, RecvGate::def(),
            ResMngOperation::OPEN_SESS, dst, name
        ).map(|_| ())
    }

    pub fn close_sess(&self, sel: Selector) -> Result<(), Error> {
        send_recv_res!(
            &self.sgate, RecvGate::def(),
            ResMngOperation::CLOSE_SESS, sel
        ).map(|_| ())
    }

    pub fn alloc_mem(&self, dst: Selector, addr: goff,
                     size: usize, perms: kif::Perm) -> Result<(), Error> {
        send_recv_res!(
            &self.sgate, RecvGate::def(),
            ResMngOperation::ALLOC_MEM, dst, addr, size, perms.bits()
        ).map(|_| ())
    }

    pub fn free_mem(&self, sel: Selector) -> Result<(), Error> {
        send_recv_res!(
            &self.sgate, RecvGate::def(),
            ResMngOperation::FREE_MEM, sel
        ).map(|_| ())
    }

    pub fn use_sem(&self, sel: Selector, name: &str) -> Result<(), Error> {
        send_recv_res!(
            &self.sgate, RecvGate::def(),
            ResMngOperation::USE_SEM, sel, name
        ).map(|_| ())
    }
}

impl Drop for ResMng {
    fn drop(&mut self) {
        if self.vpe_sel != kif::INVALID_SEL {
            send_recv_res!(
                &VPE::cur().resmng().sgate, RecvGate::def(),
                ResMngOperation::REM_CHILD, self.vpe_sel
            ).ok();
        }
    }
}
