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

use arch::pexcalls;
use core::intrinsics;
use dtu::{self, CmdFlags, EpId, Header, Label, Message, EP_COUNT};
use errors::{Code, Error};
use goff;

pub struct DTUIf {}

#[cfg(target_os = "none")]
const USE_PEXCALLS: bool = true;
#[cfg(target_os = "linux")]
const USE_PEXCALLS: bool = false;

impl DTUIf {
    fn addr_to_msg(addr: usize) -> &'static Message {
        unsafe {
            let head = addr as usize as *const Header;
            let slice = [addr as usize, (*head).length as usize];
            intrinsics::transmute(slice)
        }
    }

    #[inline(always)]
    pub fn send(
        ep: EpId,
        msg: *const u8,
        size: usize,
        reply_lbl: Label,
        reply_ep: EpId,
    ) -> Result<(), Error> {
        if USE_PEXCALLS {
            pexcalls::call5(
                pexcalls::Operation::SEND,
                ep,
                msg as usize,
                size,
                reply_lbl as usize,
                reply_ep,
            )
            .map(|_| ())
        }
        else {
            dtu::DTU::send(ep, msg, size, reply_lbl, reply_ep)
        }
    }

    #[inline(always)]
    pub fn reply(
        ep: EpId,
        reply: *const u8,
        size: usize,
        msg: &'static Message,
    ) -> Result<(), Error> {
        if USE_PEXCALLS {
            pexcalls::call4(
                pexcalls::Operation::REPLY,
                ep,
                reply as usize,
                size,
                msg as *const Message as *const u8 as usize,
            )
            .map(|_| ())
        }
        else {
            dtu::DTU::reply(ep, reply, size, msg)
        }
    }

    #[inline(always)]
    pub fn call(
        ep: EpId,
        msg: *const u8,
        size: usize,
        reply_ep: EpId,
    ) -> Result<&'static Message, Error> {
        if USE_PEXCALLS {
            pexcalls::call4(pexcalls::Operation::CALL, ep, msg as usize, size, reply_ep)
                .map(|m| Self::addr_to_msg(m))
        }
        else {
            Self::send(ep, msg, size, 0, reply_ep)?;
            Self::receive(reply_ep, Some(ep))
        }
    }

    #[inline(always)]
    pub fn fetch_msg(ep: EpId) -> Option<&'static Message> {
        if USE_PEXCALLS {
            pexcalls::call1(pexcalls::Operation::FETCH, ep)
                .ok()
                .map(|m| Self::addr_to_msg(m))
        }
        else {
            dtu::DTU::fetch_msg(ep)
        }
    }

    #[inline(always)]
    pub fn mark_read(ep: EpId, msg: &Message) {
        if USE_PEXCALLS {
            pexcalls::call2(
                pexcalls::Operation::ACK,
                ep,
                msg as *const Message as *const u8 as usize,
            )
            .ok();
        }
        else {
            dtu::DTU::mark_read(ep, msg)
        }
    }

    pub fn receive(rep: EpId, sep: Option<EpId>) -> Result<&'static Message, Error> {
        if USE_PEXCALLS {
            let sep = match sep {
                Some(ep) => ep,
                None => EP_COUNT,
            };
            pexcalls::call2(pexcalls::Operation::RECV, rep, sep).map(|m| Self::addr_to_msg(m))
        }
        else {
            loop {
                let msg = dtu::DTU::fetch_msg(rep);
                if let Some(m) = msg {
                    return Ok(m);
                }

                // fetch the events first
                dtu::DTU::fetch_events();
                if let Some(ep) = sep {
                    // now check whether the endpoint is still valid. if the EP has been invalidated
                    // before the line above, we'll notice that with this check. if the EP is
                    // invalidated between the line above and the sleep command, the DTU will refuse
                    // to suspend the core.
                    if !dtu::DTU::is_valid(ep) {
                        return Err(Error::new(Code::InvEP));
                    }
                }

                dtu::DTU::sleep()?;
            }
        }
    }

    pub fn read(
        ep: EpId,
        data: *mut u8,
        size: usize,
        off: goff,
        flags: CmdFlags,
    ) -> Result<(), Error> {
        if USE_PEXCALLS {
            pexcalls::call5(
                pexcalls::Operation::READ,
                ep,
                data as usize,
                size,
                off as usize,
                flags.bits() as usize,
            )
            .map(|_| ())
        }
        else {
            dtu::DTU::read(ep, data, size, off, flags)
        }
    }

    pub fn write(
        ep: EpId,
        data: *const u8,
        size: usize,
        off: goff,
        flags: CmdFlags,
    ) -> Result<(), Error> {
        if USE_PEXCALLS {
            pexcalls::call5(
                pexcalls::Operation::WRITE,
                ep,
                data as usize,
                size,
                off as usize,
                flags.bits() as usize,
            )
            .map(|_| ())
        }
        else {
            dtu::DTU::write(ep, data, size, off, flags)
        }
    }

    #[inline(always)]
    pub fn sleep() -> Result<(), Error> {
        Self::sleep_for(0)
    }

    #[inline(always)]
    pub fn sleep_for(cycles: u64) -> Result<(), Error> {
        if USE_PEXCALLS {
            pexcalls::call1(pexcalls::Operation::SLEEP, cycles as usize).map(|_| ())
        }
        else {
            dtu::DTU::sleep_for(cycles)
        }
    }
}
