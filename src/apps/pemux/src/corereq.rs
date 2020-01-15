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
use base::kif;
use core::intrinsics;

use helper::IRQsOnGuard;
use upcalls;
use vpe;

pub fn handle_recv(req: dtu::Reg) {
    log!(crate::LOG_FOREIGN_MSG, "Got core request {:#x}", req);

    // add message to VPE
    let vpe_id = (req >> 12) & 0xFFFF;
    if let Some(v) = vpe::get_mut(vpe_id) {
        v.add_msg();
        log!(crate::LOG_FOREIGN_MSG, "Added message to VPE {} ({} msgs)", vpe_id, v.msgs());
    }

    // wait for the message if it's for us or for the current VPE
    if vpe_id == kif::pemux::VPE_ID || vpe_id == vpe::cur().id() {
        // get number of messages
        let ep_id = (req >> 28) as dtu::EpId;
        let unread_mask = dtu::DTU::unread_mask(ep_id);
        unsafe { intrinsics::atomic_fence() };

        // let the DTU continue the message reception
        dtu::DTU::set_core_resp(req);

        // ignore upcalls during nested interrupts; we'll handle them as soon as we're done here
        // (otherwise this could steal the message that we're waiting on here)
        // TODO what about pagefaults in the meantime?
        upcalls::disable();

        // we need to enable interrupts to allow address translations for message reception
        let _guard = IRQsOnGuard::new();

        // wait here until the message has been received
        // (otherwise fetching it afterwards might fail)
        while dtu::DTU::unread_mask(ep_id) == unread_mask {
        }

        upcalls::enable();
    }
    else {
        dtu::DTU::set_core_resp(req);
    }
}
