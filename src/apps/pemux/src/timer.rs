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

use base::cell::StaticCell;
use base::col::Vec;
use base::tcu;

use vpe;

struct Timeout {
    time: u64,
    vpe: u64,
}

static LIST: StaticCell<Vec<Timeout>> = StaticCell::new(Vec::new());

pub fn add(vpe: u64, delay_ns: u64) {
    let now = tcu::TCU::nanotime();
    let timeout = Timeout {
        time: now + delay_ns,
        vpe,
    };

    log!(
        crate::LOG_TIMER,
        "Blocking VPE {} for {} ns (until {} ns)",
        vpe,
        delay_ns,
        timeout.time
    );

    if let Some(idx) = LIST.iter().position(|t| t.time < timeout.time) {
        LIST.get_mut().insert(idx, timeout);
    }
    else {
        tcu::TCU::set_timer(delay_ns);
        LIST.get_mut().push(timeout);
    }
}

pub fn remove(vpe: u64) {
    log!(crate::LOG_TIMER, "Removing VPE {}", vpe);
    LIST.get_mut().retain(|t| t.vpe != vpe);
    if LIST.is_empty() {
        tcu::TCU::set_timer(0);
    }
}

pub fn trigger() {
    if LIST.is_empty() {
        return;
    }

    let now = tcu::TCU::nanotime();
    while !LIST.is_empty() && now >= LIST[LIST.len() - 1].time {
        let timeout = LIST.get_mut().pop().unwrap();
        log!(crate::LOG_TIMER, "Unblocking VPE {} @ {}", timeout.vpe, now);
        vpe::get_mut(timeout.vpe).unwrap().unblock(None, true);
    }

    if !LIST.is_empty() {
        tcu::TCU::set_timer(LIST[LIST.len() - 1].time - now);
    }
}
