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
use base::kif;
use base::tcu;
use core::cmp;

use vpe;

pub type Nanos = u64;

struct Timeout {
    time: Nanos,
    vpe: vpe::Id,
}

static LIST: StaticCell<Vec<Timeout>> = StaticCell::new(Vec::new());

pub fn add(vpe: vpe::Id, duration: Nanos) {
    let timeout = Timeout {
        time: tcu::TCU::nanotime() + duration,
        vpe,
    };

    log!(
        crate::LOG_TIMER,
        "timer: blocking VPE {} for {} ns (until {} ns)",
        vpe,
        duration,
        timeout.time
    );

    // insert new timeout in descending order of timeouts
    if let Some(idx) = LIST.iter().position(|t| t.time < timeout.time) {
        LIST.get_mut().insert(idx, timeout);
    }
    else {
        LIST.get_mut().push(timeout);
        reprogram();
    }
}

pub fn remove(vpe: vpe::Id) {
    log!(crate::LOG_TIMER, "timer: removing VPE {}", vpe);
    LIST.get_mut().retain(|t| t.vpe != vpe);
    reprogram();
}

pub fn reprogram() {
    // determine the remaining budget of the current VPE, if there is any
    let budget = vpe::try_cur().and_then(|cur| {
        // don't use a budget if there is no ready VPE or we're idling
        if vpe::has_ready() && cur.id() != kif::pemux::IDLE_ID {
            Some(cur.budget_left())
        }
        else {
            None
        }
    });

    // determine timeout to program
    let timeout = match (LIST.is_empty(), budget) {
        // no timeout programmed: use the budget
        (true, Some(b)) => b,
        // no timeout and no budget: disable timer
        (true, None) => 0,
        // timeout: program the earlier point in time
        (false, _) => {
            let timeout = LIST[LIST.len() - 1].time - tcu::TCU::nanotime();
            cmp::min(timeout, budget.unwrap_or(Nanos::max_value()))
        }
    };

    log!(crate::LOG_TIMER, "timer: setting timer to {}", timeout);
    tcu::TCU::set_timer(timeout);
}

pub fn trigger() {
    if LIST.is_empty() {
        return;
    }

    // unblock all VPEs whose timeouts are due
    let now = tcu::TCU::nanotime();
    while !LIST.is_empty() && now >= LIST[LIST.len() - 1].time {
        let timeout = LIST.get_mut().pop().unwrap();
        log!(
            crate::LOG_TIMER,
            "timer: unblocking VPE {} @ {}",
            timeout.vpe,
            now
        );
        vpe::get_mut(timeout.vpe).unwrap().unblock(None, true);
    }

    // if a scheduling is pending, we can skip this step here, because we'll do it later anyway
    if !crate::scheduling_pending() {
        reprogram();
    }
}
