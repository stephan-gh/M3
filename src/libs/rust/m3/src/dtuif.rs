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

use arch::env;
use arch::pexcalls;
use base::pexif;
use com::{MemGate, RecvGate, SendGate};
use dtu::{self, CmdFlags, Label, Message};
use errors::{Code, Error};
use goff;

pub struct DTUIf {}

impl DTUIf {
    #[inline(always)]
    pub fn send(
        sg: &SendGate,
        msg: *const u8,
        size: usize,
        reply_lbl: Label,
        rg: &RecvGate,
    ) -> Result<(), Error> {
        let ep = sg.activate()?;
        dtu::DTU::send(ep.id(), msg, size, reply_lbl, rg.ep().unwrap())
    }

    #[inline(always)]
    pub fn reply(
        rg: &RecvGate,
        reply: *const u8,
        size: usize,
        msg: &'static Message,
    ) -> Result<(), Error> {
        dtu::DTU::reply(rg.ep().unwrap(), reply, size, msg)
    }

    #[inline(always)]
    pub fn call(
        sg: &SendGate,
        msg: *const u8,
        size: usize,
        rg: &RecvGate,
    ) -> Result<&'static Message, Error> {
        let ep = sg.activate()?;
        dtu::DTU::send(ep.id(), msg, size, 0, rg.ep().unwrap())?;
        Self::receive(rg, Some(sg))
    }

    #[inline(always)]
    pub fn fetch_msg(rg: &RecvGate) -> Option<&'static Message> {
        dtu::DTU::fetch_msg(rg.ep().unwrap())
    }

    #[inline(always)]
    pub fn ack_msg(rg: &RecvGate, msg: &Message) {
        dtu::DTU::ack_msg(rg.ep().unwrap(), msg)
    }

    pub fn receive(rg: &RecvGate, sg: Option<&SendGate>) -> Result<&'static Message, Error> {
        loop {
            let msg = dtu::DTU::fetch_msg(rg.ep().unwrap());
            if let Some(m) = msg {
                return Ok(m);
            }

            // fetch the events first
            dtu::DTU::fetch_events();
            if let Some(sg) = sg {
                // now check whether the endpoint is still valid. if the EP has been invalidated
                // before the line above, we'll notice that with this check. if the EP is
                // invalidated between the line above and the sleep command, the DTU will refuse
                // to suspend the core.
                if !dtu::DTU::is_valid(sg.ep().unwrap().id()) {
                    return Err(Error::new(Code::InvEP));
                }
            }

            dtu::DTU::wait_for_msg(rg.ep().unwrap(), 0)?;
        }
    }

    pub fn read(
        mg: &MemGate,
        data: *mut u8,
        size: usize,
        off: goff,
        flags: CmdFlags,
    ) -> Result<(), Error> {
        let ep = mg.activate()?;
        dtu::DTU::read(ep.id(), data, size, off, flags)
    }

    pub fn write(
        mg: &MemGate,
        data: *const u8,
        size: usize,
        off: goff,
        flags: CmdFlags,
    ) -> Result<(), Error> {
        let ep = mg.activate()?;
        dtu::DTU::write(ep.id(), data, size, off, flags)
    }

    #[inline(always)]
    pub fn sleep() -> Result<(), Error> {
        Self::sleep_for(0)
    }

    #[inline(always)]
    pub fn sleep_for(cycles: u64) -> Result<(), Error> {
        if env::get().shared() {
            pexcalls::call1(pexif::Operation::SLEEP, cycles as usize).map(|_| ())
        }
        else if dtu::DTU::fetch_events() == 0 {
            dtu::DTU::sleep_for(cycles)
        }
        else {
            Ok(())
        }
    }
}
