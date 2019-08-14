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

use dtu::{self, CmdFlags, EpId, Label, Message};
use errors::{Code, Error};
use goff;

pub struct DTUIf {}

impl DTUIf {
    #[inline(always)]
    pub fn send(
        ep: EpId,
        msg: *const u8,
        size: usize,
        reply_lbl: Label,
        reply_ep: EpId,
    ) -> Result<(), Error> {
        dtu::DTU::send(ep, msg, size, reply_lbl, reply_ep)
    }

    #[inline(always)]
    pub fn reply(
        ep: EpId,
        reply: *const u8,
        size: usize,
        msg: &'static Message,
    ) -> Result<(), Error> {
        dtu::DTU::reply(ep, reply, size, msg)
    }

    #[inline(always)]
    pub fn call(
        ep: EpId,
        msg: *const u8,
        size: usize,
        reply_ep: EpId,
    ) -> Result<&'static Message, Error> {
        Self::send(ep, msg, size, 0, reply_ep)?;
        Self::receive(reply_ep, Some(ep))
    }

    #[inline(always)]
    pub fn fetch_msg(ep: EpId) -> Option<&'static Message> {
        dtu::DTU::fetch_msg(ep)
    }

    #[inline(always)]
    pub fn mark_read(ep: EpId, msg: &Message) {
        dtu::DTU::mark_read(ep, msg)
    }

    pub fn receive(rep: EpId, sep: Option<EpId>) -> Result<&'static Message, Error> {
        loop {
            let msg = dtu::DTU::fetch_msg(rep);
            if let Some(m) = msg {
                return Ok(m);
            }

            // fetch the events first
            dtu::DTU::fetch_events();
            if let Some(ep) = sep {
                // now check whether the endpoint is still valid. if the EP has been invalidated before the
                // line above, we'll notice that with this check. if the EP is invalidated between the line
                // above and the sleep command, the DTU will refuse to suspend the core.
                if !dtu::DTU::is_valid(ep) {
                    return Err(Error::new(Code::InvEP));
                }
            }

            dtu::DTU::sleep()?;
        }
    }

    pub fn read(
        ep: EpId,
        data: *mut u8,
        size: usize,
        off: goff,
        flags: CmdFlags,
    ) -> Result<(), Error> {
        dtu::DTU::read(ep, data, size, off, flags)
    }

    pub fn write(
        ep: EpId,
        data: *const u8,
        size: usize,
        off: goff,
        flags: CmdFlags,
    ) -> Result<(), Error> {
        dtu::DTU::write(ep, data, size, off, flags)
    }

    #[inline(always)]
    pub fn sleep() -> Result<(), Error> {
        dtu::DTU::sleep()
    }

    #[inline(always)]
    pub fn sleep_for(cycles: u64) -> Result<(), Error> {
        dtu::DTU::sleep_for(cycles)
    }
}
