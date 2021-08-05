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

use core::slice;

use base::errors::{Code, Error};
use base::goff;
use base::kif;
use base::log;
use base::mem::GlobAddr;
use base::pexif;
use base::tcu::{EpId, INVALID_EP, IRQ, TCU};

use crate::irqs;
use crate::timer;
use crate::vma;
use crate::vpe;
use crate::{arch, helper};

fn pexcall_wait(state: &mut arch::State) -> Result<(), Error> {
    let ep = state.r[isr::PEXC_ARG1] as EpId;
    let irq = state.r[isr::PEXC_ARG2] as pexif::IRQId;
    let timeout = state.r[isr::PEXC_ARG3] as timer::Nanos;

    log!(
        crate::LOG_CALLS,
        "pexcall::wait(ep={}, irq={}, timeout={})",
        ep,
        irq,
        timeout,
    );

    let cur = vpe::cur();
    let wait_ep = if ep == INVALID_EP { None } else { Some(ep) };
    let wait_irq = if irq <= IRQ::TIMER.val as pexif::IRQId || irq == pexif::INVALID_IRQ {
        None
    }
    else {
        Some(irq)
    };

    if wait_ep.is_none() || wait_irq.is_some() {
        if irqs::wait(cur, wait_irq).is_some() {
            return Ok(());
        }
    }

    let timeout = if timeout == 0 {
        None
    }
    else {
        timer::add(cur.id(), timeout);
        Some(timeout)
    };

    cur.block(None, wait_ep, wait_irq, timeout);

    Ok(())
}

fn pexcall_stop(state: &mut arch::State) -> Result<(), Error> {
    let code = state.r[isr::PEXC_ARG1] as u32;

    log!(crate::LOG_CALLS, "pexcall::stop(code={})", code);

    vpe::remove_cur(code);

    Ok(())
}

fn pexcall_yield(_state: &mut arch::State) -> Result<(), Error> {
    log!(crate::LOG_CALLS, "pexcall::yield()");

    if vpe::has_ready() {
        crate::reg_scheduling(vpe::ScheduleAction::Yield);
    }
    Ok(())
}

fn pexcall_map(state: &mut arch::State) -> Result<(), Error> {
    let virt = state.r[isr::PEXC_ARG1] as usize;
    let phys = state.r[isr::PEXC_ARG2] as goff;
    let pages = state.r[isr::PEXC_ARG3] as usize;
    let access = kif::Perm::from_bits_truncate(state.r[isr::PEXC_ARG4] as u32);
    let flags = kif::PageFlags::from(access) & kif::PageFlags::RW;

    log!(
        crate::LOG_CALLS,
        "pexcall::map(virt={:#x}, phys={:#x}, pages={}, access={:?})",
        virt,
        phys,
        pages,
        access
    );

    if pages == 0 || flags.is_empty() {
        return Err(Error::new(Code::InvArgs));
    }

    // TODO validate virtual and physical address

    let global = GlobAddr::new(phys);
    vpe::cur().map(virt, global, pages, flags | kif::PageFlags::U)
}

fn pexcall_reg_irq(state: &mut arch::State) -> Result<(), Error> {
    let irq = state.r[isr::PEXC_ARG1] as pexif::IRQId;

    log!(crate::LOG_CALLS, "pexcall::reg_irq(irq={:?})", irq);

    // TODO validate whether the VPE is allowed to use these IRQs

    irqs::register(vpe::cur(), irq);

    Ok(())
}

fn pexcall_transl_fault(state: &mut arch::State) -> Result<(), Error> {
    let virt = state.r[isr::PEXC_ARG1] as usize;
    let access = kif::Perm::from_bits_truncate(state.r[isr::PEXC_ARG2] as u32);
    let flags = kif::PageFlags::from(access) & kif::PageFlags::RW;

    log!(
        crate::LOG_CALLS,
        "pexcall::transl_fault(virt={:#x}, access={:?})",
        virt,
        access
    );

    vma::handle_xlate(virt, flags);

    Ok(())
}

fn pexcall_flush_inv(_state: &mut arch::State) -> Result<(), Error> {
    log!(crate::LOG_CALLS, "pexcall::flush_inv()");

    helper::flush_invalidate();

    Ok(())
}

fn pexcall_print(state: &mut arch::State) -> Result<(), Error> {
    let addr = state.r[isr::PEXC_ARG1] as usize;
    let len = state.r[isr::PEXC_ARG2] as usize;

    log!(
        crate::LOG_CALLS,
        "pexcall::print(addr={:#x}, len={})",
        addr,
        len
    );

    let old_vpe = TCU::xchg_vpe(vpe::our().vpe_reg()).unwrap();

    let mut msgbuf = base::mem::MsgBuf::borrow_def();
    msgbuf.set_from_slice(unsafe { slice::from_raw_parts(addr as *const u8, len) });
    let res = TCU::write(127, msgbuf.bytes().as_ptr(), len, 0);

    // change back to old VPE
    let our_vpe = TCU::xchg_vpe(old_vpe).unwrap();
    assert!((our_vpe >> 16) == 0);
    vpe::our().set_vpe_reg(our_vpe);

    res
}

fn pexcall_noop(_state: &mut arch::State) -> Result<(), Error> {
    log!(crate::LOG_CALLS, "pexcall::noop()");

    Ok(())
}

pub fn handle_call(state: &mut arch::State) {
    let call = pexif::Operation::from(state.r[isr::PEXC_ARG0] as isize);

    let res = match call {
        pexif::Operation::WAIT => pexcall_wait(state).map(|_| 0isize),
        pexif::Operation::EXIT => pexcall_stop(state).map(|_| 0isize),
        pexif::Operation::YIELD => pexcall_yield(state).map(|_| 0isize),
        pexif::Operation::MAP => pexcall_map(state).map(|_| 0isize),
        pexif::Operation::REG_IRQ => pexcall_reg_irq(state).map(|_| 0isize),
        pexif::Operation::TRANSL_FAULT => pexcall_transl_fault(state).map(|_| 0isize),
        pexif::Operation::FLUSH_INV => pexcall_flush_inv(state).map(|_| 0isize),
        pexif::Operation::PRINT => pexcall_print(state).map(|_| 0isize),
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

    state.r[isr::PEXC_ARG0] = res.unwrap_or_else(|e| -(e.code() as isize)) as usize;
}
