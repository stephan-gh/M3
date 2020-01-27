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
use base::pexif;

use arch;
use helper;

fn pexcall_sleep(state: &mut arch::State) -> Result<(), Error> {
    let cycles = state.r[arch::PEXC_ARG1];

    log!(crate::LOG_CALLS, "pexcall::sleep(cycles={})", cycles);

    if dtu::DTU::fetch_events() == 0 {
        let _irqs = helper::IRQsOnGuard::new();
        dtu::DTU::sleep_for(cycles as u64)
    }
    else {
        Ok(())
    }
}

fn pexcall_stop(state: &mut arch::State) -> Result<(), Error> {
    log!(crate::LOG_CALLS, "pexcall::stop()");

    state.stop();
    Ok(())
}

pub fn handle_call(state: &mut arch::State) {
    let call = pexif::Operation::from(state.r[arch::PEXC_ARG0] as isize);

    let res = match call {
        pexif::Operation::SLEEP => pexcall_sleep(state).map(|_| 0isize),
        pexif::Operation::EXIT => pexcall_stop(state).map(|_| 0isize),

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
