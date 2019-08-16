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
use base::goff;
use base::pexif;
use core::intrinsics;
use isr;

fn addr_to_msg(addr: usize) -> &'static dtu::Message {
    unsafe {
        let head = addr as *const dtu::Header;
        let slice = [addr, (*head).length as usize];
        intrinsics::transmute(slice)
    }
}

fn recv_msg(rep: dtu::EpId, sep: dtu::EpId) -> Result<isize, Error> {
    while !isr::is_stopped() {
        let msg = dtu::DTU::fetch_msg(rep);
        if let Some(m) = msg {
            return Ok(m as *const dtu::Message as *const u8 as isize);
        }

        // fetch the events first
        dtu::DTU::fetch_events();
        // now check whether the endpoint is still valid. if the EP has been invalidated
        // before the line above, we'll notice that with this check. if the EP is
        // invalidated between the line above and the sleep command, the DTU will refuse
        // to suspend the core.
        if sep != dtu::EP_COUNT && !dtu::DTU::is_valid(sep) {
            return Err(Error::new(Code::InvEP));
        }

        dtu::DTU::sleep()?;
    }
    Err(Error::new(Code::Abort))
}

fn pexcall_send(state: &mut isr::State) -> Result<(), Error> {
    let ep = state.r[isr::PEXC_ARG1] as dtu::EpId;
    let msg = state.r[isr::PEXC_ARG2] as *const u8;
    let size = state.r[isr::PEXC_ARG3];
    let reply_lbl = state.r[isr::PEXC_ARG4] as dtu::Label;
    let reply_ep = state.r[isr::PEXC_ARG5] as dtu::EpId;
    dtu::DTU::send(ep, msg, size, reply_lbl, reply_ep)
}

fn pexcall_reply(state: &mut isr::State) -> Result<(), Error> {
    let ep = state.r[isr::PEXC_ARG1] as dtu::EpId;
    let reply = state.r[isr::PEXC_ARG2] as *const u8;
    let size = state.r[isr::PEXC_ARG3];
    let msg = state.r[isr::PEXC_ARG4];
    dtu::DTU::reply(ep, reply, size, addr_to_msg(msg))
}

fn pexcall_call(state: &mut isr::State) -> Result<isize, Error> {
    let sep = state.r[isr::PEXC_ARG1] as dtu::EpId;
    let msg = state.r[isr::PEXC_ARG2] as *const u8;
    let size = state.r[isr::PEXC_ARG3];
    let rep = state.r[isr::PEXC_ARG4] as dtu::EpId;
    dtu::DTU::send(sep, msg, size, 0, rep)?;
    recv_msg(rep, sep)
}

fn pexcall_recv(state: &mut isr::State) -> Result<isize, Error> {
    let rep = state.r[isr::PEXC_ARG1] as dtu::EpId;
    let sep = state.r[isr::PEXC_ARG2] as dtu::EpId;
    recv_msg(rep, sep)
}

fn pexcall_fetch(state: &mut isr::State) -> Result<isize, Error> {
    let ep = state.r[isr::PEXC_ARG1] as dtu::EpId;
    match dtu::DTU::fetch_msg(ep) {
        None => Err(Error::new(Code::InvArgs)), // error is not used
        Some(addr) => Ok(addr as *const dtu::Message as *const u8 as isize),
    }
}

fn pexcall_ack(state: &mut isr::State) -> Result<(), Error> {
    let ep = state.r[isr::PEXC_ARG1] as dtu::EpId;
    let msg = addr_to_msg(state.r[isr::PEXC_ARG2]);
    dtu::DTU::mark_read(ep, msg);
    Ok(())
}

fn pexcall_read(state: &mut isr::State) -> Result<(), Error> {
    let ep = state.r[isr::PEXC_ARG1] as dtu::EpId;
    let data = state.r[isr::PEXC_ARG2] as *mut u8;
    let size = state.r[isr::PEXC_ARG3];
    let off = state.r[isr::PEXC_ARG4] as goff;
    let flags = dtu::CmdFlags::from_bits_truncate(state.r[isr::PEXC_ARG5] as u64);
    dtu::DTU::read(ep, data, size, off, flags)
}

fn pexcall_write(state: &mut isr::State) -> Result<(), Error> {
    let ep = state.r[isr::PEXC_ARG1] as dtu::EpId;
    let data = state.r[isr::PEXC_ARG2] as *const u8;
    let size = state.r[isr::PEXC_ARG3];
    let off = state.r[isr::PEXC_ARG4] as goff;
    let flags = dtu::CmdFlags::from_bits_truncate(state.r[isr::PEXC_ARG5] as u64);
    dtu::DTU::write(ep, data, size, off, flags)
}

fn pexcall_sleep(state: &mut isr::State) -> Result<(), Error> {
    let cycles = state.r[isr::PEXC_ARG1];
    dtu::DTU::sleep_for(cycles as u64)
}

fn pexcall_stop(state: &mut isr::State) -> Result<(), Error> {
    state.stop();
    Ok(())
}

pub fn handle_call(state: &mut isr::State) {
    let call = pexif::Operation::from(state.r[isr::PEXC_ARG0] as isize);

    // log!(DEF, "Got PEXCall {}", call);
    // log!(DEF, " Arg1 = {:#x}", { state.r[isr::PEXC_ARG1] });
    // log!(DEF, " Arg2 = {:#x}", { state.r[isr::PEXC_ARG2] });
    // log!(DEF, " Arg3 = {:#x}", { state.r[isr::PEXC_ARG3] });
    // log!(DEF, " Arg4 = {:#x}", { state.r[isr::PEXC_ARG4] });
    // log!(DEF, " Arg5 = {:#x}", { state.r[isr::PEXC_ARG5] });

    // enable interrupts in case we need to translate addresses for the DTU
    isr::toggle_ints(true);

    let res = match call {
        pexif::Operation::EXIT => pexcall_stop(state).map(|_| 0isize),
        pexif::Operation::SEND => pexcall_send(state).map(|_| 0isize),
        pexif::Operation::REPLY => pexcall_reply(state).map(|_| 0isize),
        pexif::Operation::CALL => pexcall_call(state),
        pexif::Operation::FETCH => pexcall_fetch(state),
        pexif::Operation::RECV => pexcall_recv(state),
        pexif::Operation::ACK => pexcall_ack(state).map(|_| 0isize),
        pexif::Operation::READ => pexcall_read(state).map(|_| 0isize),
        pexif::Operation::WRITE => pexcall_write(state).map(|_| 0isize),
        pexif::Operation::SLEEP => pexcall_sleep(state).map(|_| 0isize),
        _ => Err(Error::new(Code::NotSup)),
    };

    isr::toggle_ints(false);

    state.r[isr::PEXC_ARG0] = res.unwrap_or_else(|e| -(e.code() as isize)) as usize;
}
