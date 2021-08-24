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

//! Contains the interface between applications and PEMux

use crate::arch::pexabi;
use crate::errors::Error;
use crate::goff;
use crate::kif;
use crate::tcu::{EpId, INVALID_EP};

pub type IRQId = u32;

pub const INVALID_IRQ: IRQId = !0;

int_enum! {
    /// The operations PEMux supports
    pub struct Operation : isize {
        /// Wait for an event, optionally with timeout
        const WAIT          = 0x0;
        /// Exit the application
        const EXIT          = 0x1;
        /// Switch to the next ready VPE
        const YIELD         = 0x2;
        /// Map local physical memory (IO memory)
        const MAP           = 0x3;
        /// Register for a given interrupt
        const REG_IRQ       = 0x4;
        /// For TCU TLB misses
        const TRANSL_FAULT  = 0x5;
        /// Flush and invalidate cache
        const FLUSH_INV     = 0x6;
        /// Read from serial device (only on hw)
        const READ_SERIAL   = 0x7;
        /// Noop operation for testing purposes
        const NOOP          = 0x8;
    }
}

#[cfg(not(target_vendor = "host"))]
pub(crate) fn get_result(res: isize) -> Result<usize, Error> {
    match res {
        e if e < 0 => Err(Error::from(-e as u32)),
        val => Ok(val as usize),
    }
}

#[inline(always)]
pub fn wait(ep: Option<EpId>, irq: Option<IRQId>, nanos: Option<u64>) -> Result<(), Error> {
    pexabi::call3(
        Operation::WAIT,
        ep.unwrap_or(INVALID_EP) as usize,
        irq.unwrap_or(INVALID_IRQ) as usize,
        nanos.unwrap_or(0) as usize,
    )
    .map(|_| ())
}

pub fn exit(code: i32) -> ! {
    pexabi::call1(Operation::EXIT, code as usize).ok();
    unreachable!();
}

pub fn map(virt: usize, phys: goff, pages: usize, access: kif::Perm) -> Result<(), Error> {
    pexabi::call4(
        Operation::MAP,
        virt,
        phys as usize,
        pages,
        access.bits() as usize,
    )
    .map(|_| ())
}

pub fn reg_irq(irq: IRQId) -> Result<(), Error> {
    pexabi::call1(Operation::REG_IRQ, irq as usize).map(|_| ())
}

pub fn flush_invalidate() -> Result<(), Error> {
    pexabi::call1(Operation::FLUSH_INV, 0).map(|_| ())
}

#[inline(always)]
pub fn switch_vpe() -> Result<(), Error> {
    pexabi::call1(Operation::YIELD, 0).map(|_| ())
}

pub fn read_serial(buf: &mut [u8]) -> Result<usize, Error> {
    pexabi::call2(Operation::READ_SERIAL, buf.as_mut_ptr() as usize, buf.len())
}

#[inline(always)]
pub fn noop() -> Result<(), Error> {
    pexabi::call1(Operation::NOOP, 0).map(|_| ())
}
