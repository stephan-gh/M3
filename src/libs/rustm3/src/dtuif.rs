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
use base::pexif;
use com::{EpMux, Gate, MemGate, RecvGate, SendGate};
use core::intrinsics;
use dtu::{self, CmdFlags, Header, Label, Message, EP_COUNT};
use errors::{Code, Error};
use goff;
use kif;
use syscalls;
use vpe::VPE;

pub struct DTUIf {}

#[cfg(target_os = "none")]
pub(crate) const USE_PEXCALLS: bool = true;
#[cfg(target_os = "linux")]
pub(crate) const USE_PEXCALLS: bool = false;

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
        sg: &SendGate,
        msg: *const u8,
        size: usize,
        reply_lbl: Label,
        rg: &RecvGate,
    ) -> Result<(), Error> {
        if USE_PEXCALLS {
            pexcalls::call5(
                pexif::Operation::SEND,
                sg.sel() as usize,
                msg as usize,
                size,
                reply_lbl as usize,
                rg.sel() as usize,
            )
            .map(|_| ())
        }
        else {
            let ep = sg.activate()?;
            dtu::DTU::send(ep, msg, size, reply_lbl, rg.ep().unwrap())
        }
    }

    #[inline(always)]
    pub fn reply(
        rg: &RecvGate,
        reply: *const u8,
        size: usize,
        msg: &'static Message,
    ) -> Result<(), Error> {
        if USE_PEXCALLS {
            pexcalls::call4(
                pexif::Operation::REPLY,
                rg.sel() as usize,
                reply as usize,
                size,
                msg as *const Message as *const u8 as usize,
            )
            .map(|_| ())
        }
        else {
            dtu::DTU::reply(rg.ep().unwrap(), reply, size, msg)
        }
    }

    #[inline(always)]
    pub fn call(
        sg: &SendGate,
        msg: *const u8,
        size: usize,
        rg: &RecvGate,
    ) -> Result<&'static Message, Error> {
        if USE_PEXCALLS {
            pexcalls::call4(
                pexif::Operation::CALL,
                sg.sel() as usize,
                msg as usize,
                size,
                rg.sel() as usize,
            )
            .map(|m| Self::addr_to_msg(m))
        }
        else {
            let ep = sg.activate()?;
            dtu::DTU::send(ep, msg, size, 0, rg.ep().unwrap())?;
            Self::receive(rg, Some(sg))
        }
    }

    #[inline(always)]
    pub fn fetch_msg(rg: &RecvGate) -> Option<&'static Message> {
        if USE_PEXCALLS {
            pexcalls::call1(pexif::Operation::FETCH, rg.sel() as usize)
                .ok()
                .map(|m| Self::addr_to_msg(m))
        }
        else {
            dtu::DTU::fetch_msg(rg.ep().unwrap())
        }
    }

    #[inline(always)]
    pub fn mark_read(rg: &RecvGate, msg: &Message) {
        if USE_PEXCALLS {
            pexcalls::call2(
                pexif::Operation::ACK,
                rg.sel() as usize,
                msg as *const Message as *const u8 as usize,
            )
            .ok();
        }
        else {
            dtu::DTU::mark_read(rg.ep().unwrap(), msg)
        }
    }

    pub fn receive(rg: &RecvGate, sg: Option<&SendGate>) -> Result<&'static Message, Error> {
        if USE_PEXCALLS {
            let sgsel = match sg {
                Some(sg) => sg.sel() as usize,
                None => kif::INVALID_SEL as usize,
            };
            pexcalls::call2(pexif::Operation::RECV, rg.sel() as usize, sgsel)
                .map(|m| Self::addr_to_msg(m))
        }
        else {
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
                    if !dtu::DTU::is_valid(sg.ep().unwrap()) {
                        return Err(Error::new(Code::InvEP));
                    }
                }

                dtu::DTU::sleep()?;
            }
        }
    }

    fn mgate_sel(mg: &MemGate) -> usize {
        match mg.sel() {
            kif::INVALID_SEL => 1 << 31 | mg.ep().unwrap(),
            sel => sel as usize,
        }
    }

    pub fn read(
        mg: &MemGate,
        data: *mut u8,
        size: usize,
        off: goff,
        flags: CmdFlags,
    ) -> Result<(), Error> {
        if USE_PEXCALLS {
            pexcalls::call5(
                pexif::Operation::READ,
                Self::mgate_sel(mg),
                data as usize,
                size,
                off as usize,
                flags.bits() as usize,
            )
            .map(|_| ())
        }
        else {
            let ep = mg.activate()?;
            dtu::DTU::read(ep, data, size, off, flags)
        }
    }

    pub fn write(
        mg: &MemGate,
        data: *const u8,
        size: usize,
        off: goff,
        flags: CmdFlags,
    ) -> Result<(), Error> {
        if USE_PEXCALLS {
            pexcalls::call5(
                pexif::Operation::WRITE,
                Self::mgate_sel(mg),
                data as usize,
                size,
                off as usize,
                flags.bits() as usize,
            )
            .map(|_| ())
        }
        else {
            let ep = mg.activate()?;
            dtu::DTU::write(ep, data, size, off, flags)
        }
    }

    pub fn reserve_ep(ep: Option<dtu::EpId>) -> Result<dtu::EpId, Error> {
        assert!(USE_PEXCALLS);

        let ep = match ep {
            Some(id) => id,
            None => EP_COUNT,
        };
        pexcalls::call1(pexif::Operation::RES_EP, ep)
    }

    pub fn free_ep(ep: dtu::EpId) -> Result<(), Error> {
        assert!(USE_PEXCALLS);

        pexcalls::call1(pexif::Operation::FREE_EP, ep).map(|_| ())
    }

    pub fn activate_gate(gate: &Gate, ep: dtu::EpId, addr: goff) -> Result<(), Error> {
        if USE_PEXCALLS {
            pexcalls::call3(
                pexif::Operation::ACTIVATE_GATE,
                gate.sel() as usize,
                ep as usize,
                addr as usize,
            )
            .map(|_| ())
        }
        else {
            let ep_sel = VPE::cur().ep_sel(ep);
            syscalls::activate(ep_sel, gate.sel(), addr)
        }
    }

    pub fn remove_gate(gate: &Gate, invalidate: bool) -> Result<(), Error> {
        if USE_PEXCALLS {
            pexcalls::call2(
                pexif::Operation::REMOVE_GATE,
                gate.sel() as usize,
                invalidate as usize,
            )
            .map(|_| ())
        }
        else {
            EpMux::get().remove(gate);
            Ok(())
        }
    }

    #[inline(always)]
    pub fn sleep() -> Result<(), Error> {
        Self::sleep_for(0)
    }

    #[inline(always)]
    pub fn sleep_for(cycles: u64) -> Result<(), Error> {
        if USE_PEXCALLS {
            pexcalls::call1(pexif::Operation::SLEEP, cycles as usize).map(|_| ())
        }
        else {
            dtu::DTU::sleep_for(cycles)
        }
    }
}
