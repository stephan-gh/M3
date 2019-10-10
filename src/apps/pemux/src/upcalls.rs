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

use base::dtu;
use base::errors::{Code, Error};
use base::kif;
use base::util;

use IRQsOnGuard;
use eps;
use vpe;

fn reply_msg<T>(msg: &'static dtu::Message, reply: &T) {
    let _irqs = IRQsOnGuard::new();
    dtu::DTU::reply(
        dtu::PEXUP_REP,
        reply as *const T as *const u8,
        util::size_of::<T>(),
        msg,
    )
    .unwrap();
}

fn alloc_ep(msg: &'static dtu::Message) -> Result<(), Error> {
    let req = &unsafe { &*(&msg.data as *const [u8] as *const [kif::pemux::AllocEP]) }[0];

    let vpe = req.vpe_sel;

    log!(PEX_UPCALLS, "alloc_ep(vpe={})", vpe);

    let ep = eps::get().find_free(false)?;
    log!(PEX_EPS, "VPE{}: reserving EP {}", vpe, ep);
    eps::get().mark_reserved(ep, vpe);

    let reply = kif::pemux::AllocEPReply {
        error: 0,
        ep: ep as u64,
    };
    reply_msg(msg, &reply);
    Ok(())
}

fn free_ep(msg: &'static dtu::Message) -> Result<(), Error> {
    let req = &unsafe { &*(&msg.data as *const [u8] as *const [kif::pemux::FreeEP]) }[0];

    let ep = req.ep as dtu::EpId;

    log!(PEX_UPCALLS, "free_ep(ep={})", ep);

    if let Some((vpe, gate)) = eps::get().mark_unreserved(ep) {
        if let Some(vpe) = vpe::get_vpe(vpe) {
            vpe.remove_gate(gate, true);
        }
    }

    reply_msg(msg, &kif::DefaultReply { error: 0 });
    Ok(())
}

fn handle_upcall(msg: &'static dtu::Message) {
    let req = &unsafe { &*(&msg.data as *const [u8] as *const [kif::DefaultRequest]) }[0];

    let res = match kif::pemux::Upcalls::from(req.opcode) {
        kif::pemux::Upcalls::ALLOC_EP => alloc_ep(msg),
        kif::pemux::Upcalls::FREE_EP => free_ep(msg),
        _ => Err(Error::new(Code::NotSup)),
    };

    if let Err(e) = res {
        let reply = kif::DefaultReply {
            error: e.code() as u64,
        };
        reply_msg(msg, &reply);
    }
}

pub fn check() {
    let msg = dtu::DTU::fetch_msg(dtu::PEXUP_REP);
    if let Some(m) = msg {
        handle_upcall(m);
    }
}
