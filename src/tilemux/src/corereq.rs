/*
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

use base::kif;
use base::log;
use base::tcu;

use crate::activities;

pub fn handle_recv(req: tcu::CoreForeignReq) {
    // add message to activity
    if let Some(mut v) = activities::get_mut(req.activity() as activities::Id) {
        // if this activity is currently running, we have to update the CUR_ACT register
        if (tcu::TCU::get_cur_activity() & 0xFFFF) == req.activity() as activities::Id {
            // temporary switch to idle
            let old_act = tcu::TCU::xchg_activity(activities::idle().activity_reg()).unwrap();
            // set user event
            v.set_activity_reg(old_act);
            v.add_msg();
            // switch back
            tcu::TCU::xchg_activity(v.activity_reg()).unwrap();
        }
        // otherwise, just add it to our copy of CUR_ACT
        else {
            v.add_msg();
        }

        log!(
            crate::LOG_FOREIGN_MSG,
            "Added message to Activity {} ({} msgs)",
            req.activity(),
            v.msgs()
        );

        if v.id() != kif::tilemux::ACT_ID {
            v.unblock(activities::Event::Message(req.ep()));
        }
    }

    tcu::TCU::set_foreign_resp();
}
