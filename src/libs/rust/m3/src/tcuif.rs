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

use base::pexif;

use crate::arch::env;
use crate::arch::pexcalls;
use crate::com::{MemGate, RecvGate, SendGate};
use crate::errors::{Code, Error};
use crate::goff;
use crate::tcu::{self, Label, Message};

pub struct TCUIf {}

impl TCUIf {
    #[inline(always)]
    pub fn send(
        sg: &SendGate,
        msg: *const u8,
        size: usize,
        reply_lbl: Label,
        rg: &RecvGate,
    ) -> Result<(), Error> {
        let ep = sg.activate()?;
        tcu::TCU::send(ep.id(), msg, size, reply_lbl, rg.ep().unwrap())
    }

    #[inline(always)]
    pub fn reply(
        rg: &RecvGate,
        reply: *const u8,
        size: usize,
        msg: &'static Message,
    ) -> Result<(), Error> {
        let off = tcu::TCU::msg_to_offset(rg.address().unwrap(), msg);
        tcu::TCU::reply(rg.ep().unwrap(), reply, size, off)
    }

    #[inline(always)]
    pub fn call(
        sg: &SendGate,
        msg: *const u8,
        size: usize,
        rg: &RecvGate,
    ) -> Result<&'static Message, Error> {
        let ep = sg.activate()?;
        tcu::TCU::send(ep.id(), msg, size, 0, rg.ep().unwrap())?;
        Self::receive(rg, Some(sg))
    }

    #[inline(always)]
    pub fn fetch_msg(rg: &RecvGate) -> Option<&'static Message> {
        tcu::TCU::fetch_msg(rg.ep().unwrap())
            .map(|off| tcu::TCU::offset_to_msg(rg.address().unwrap(), off))
    }

    #[inline(always)]
    pub fn ack_msg(rg: &RecvGate, msg: &Message) -> Result<(), Error> {
        let off = tcu::TCU::msg_to_offset(rg.address().unwrap(), msg);
        tcu::TCU::ack_msg(rg.ep().unwrap(), off)
    }

    pub fn receive(rg: &RecvGate, sg: Option<&SendGate>) -> Result<&'static Message, Error> {
        let rep = rg.ep().unwrap();
        // if the PE is shared with someone else that wants to run, poll a couple of times to
        // prevent too frequent/unnecessary switches.
        let polling = if env::get().shared() { 200 } else { 1 };
        loop {
            for _ in 0..polling {
                let msg_off = tcu::TCU::fetch_msg(rep);
                if let Some(off) = msg_off {
                    return Ok(tcu::TCU::offset_to_msg(rg.address().unwrap(), off));
                }
            }

            if let Some(sg) = sg {
                if !tcu::TCU::is_valid(sg.ep().unwrap().id()) {
                    return Err(Error::new(Code::NoSEP));
                }
            }

            Self::wait_for_msg(rep)?;
        }
    }

    pub fn read(mg: &MemGate, data: *mut u8, size: usize, off: goff) -> Result<(), Error> {
        let ep = mg.activate()?;
        tcu::TCU::read(ep.id(), data, size, off)
    }

    pub fn write(mg: &MemGate, data: *const u8, size: usize, off: goff) -> Result<(), Error> {
        let ep = mg.activate()?;
        tcu::TCU::write(ep.id(), data, size, off)
    }

    #[inline(always)]
    pub fn sleep() -> Result<(), Error> {
        Self::sleep_for(0)
    }

    #[inline(always)]
    pub fn sleep_for(nanos: u64) -> Result<(), Error> {
        if env::get().shared() || nanos != 0 {
            pexcalls::call2(
                pexif::Operation::SLEEP,
                nanos as usize,
                tcu::INVALID_EP as usize,
            )
            .map(|_| ())
        }
        else {
            tcu::TCU::wait_for_msg(tcu::INVALID_EP)
        }
    }

    pub fn wait_for_msg(ep: tcu::EpId) -> Result<(), Error> {
        if env::get().shared() {
            pexcalls::call2(pexif::Operation::SLEEP, 0, ep as usize).map(|_| ())
        }
        else {
            tcu::TCU::wait_for_msg(ep)
        }
    }

    #[inline(always)]
    pub fn switch_vpe() -> Result<(), Error> {
        pexcalls::call1(pexif::Operation::YIELD, 0).map(|_| ())
    }

    pub fn flush_invalidate() -> Result<(), Error> {
        pexcalls::call1(pexif::Operation::FLUSH_INV, 0).map(|_| ())
    }

    #[inline(always)]
    pub fn noop() -> Result<(), Error> {
        pexcalls::call1(pexif::Operation::NOOP, 0).map(|_| ())
    }
}
