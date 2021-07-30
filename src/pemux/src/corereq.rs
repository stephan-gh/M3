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

use base::kif;
use base::log;
use base::tcu;

use crate::vpe;

pub fn handle_recv(req: tcu::CoreForeignReq) {
    // add message to VPE
    if let Some(v) = vpe::get_mut(req.vpe as vpe::Id) {
        // if this VPE is currently running, we have to update the CUR_VPE register
        if (tcu::TCU::get_cur_vpe() & 0xFFFF) == req.vpe as vpe::Id {
            // temporary switch to idle
            let old_vpe = tcu::TCU::xchg_vpe(vpe::idle().vpe_reg()).unwrap();
            // set user event
            v.set_vpe_reg(old_vpe);
            v.add_msg();
            // switch back
            tcu::TCU::xchg_vpe(v.vpe_reg()).unwrap();
        }
        // otherwise, just add it to our copy of CUR_VPE
        else {
            v.add_msg();
        }

        log!(
            crate::LOG_FOREIGN_MSG,
            "Added message to VPE {} ({} msgs)",
            req.vpe,
            v.msgs()
        );

        if v.id() != kif::pemux::VPE_ID {
            v.unblock(Some(vpe::Event::Message(req.ep)), false);
        }
    }

    tcu::TCU::set_foreign_resp();
}
