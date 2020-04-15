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

use base::errors::{Code, Error};
use base::pexif;
use base::tcu;

use arch;
use timer;
use vpe;

fn pexcall_sleep(state: &mut arch::State) -> Result<(), Error> {
    let delay_ns = state.r[arch::PEXC_ARG1] as u64;
    let ep = state.r[arch::PEXC_ARG2] as tcu::EpId;

    log!(crate::LOG_CALLS, "pexcall::sleep(delay_ns={}, ep={})", delay_ns, ep);

    let cur = vpe::cur();
    if delay_ns != 0 {
        timer::add(cur.id(), delay_ns);
    }
    let wait_ep = if ep == tcu::INVALID_EP { None } else { Some(ep) };
    cur.block(vpe::ScheduleAction::Block, None, wait_ep);

    Ok(())
}

fn pexcall_stop(state: &mut arch::State) -> Result<(), Error> {
    let code = state.r[arch::PEXC_ARG1] as u32;

    log!(crate::LOG_CALLS, "pexcall::stop(code={})", code);

    vpe::remove_cur(code);

    Ok(())
}

fn pexcall_yield(_state: &mut arch::State) -> Result<(), Error> {
    log!(crate::LOG_CALLS, "pexcall::yield()");

    if vpe::has_ready() {
        crate::reg_scheduling(vpe::ScheduleAction::Preempt);
    }
    Ok(())
}

fn pexcall_noop(_state: &mut arch::State) -> Result<(), Error> {
    log!(crate::LOG_CALLS, "pexcall::noop()");

    Ok(())
}

pub fn handle_call(state: &mut arch::State) {
    let call = pexif::Operation::from(state.r[arch::PEXC_ARG0] as isize);

    let res = match call {
        pexif::Operation::SLEEP => pexcall_sleep(state).map(|_| 0isize),
        pexif::Operation::EXIT => pexcall_stop(state).map(|_| 0isize),
        pexif::Operation::YIELD => pexcall_yield(state).map(|_| 0isize),
        pexif::Operation::NOOP => pexcall_noop(state).map(|_| 0isize),

        _ => Err(Error::new(Code::NotSup)),
    };

    if let Err(e) = &res {
        log!(
            crate::LOG_CALLS,
            "\x1B[1mError for call {:?}: {:?}\x1B[0m",
            call,
            e.code()
        );
    }

    state.r[arch::PEXC_ARG0] = res.unwrap_or_else(|e| -(e.code() as isize)) as usize;
}
