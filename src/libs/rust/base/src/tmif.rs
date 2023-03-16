/*
 * Copyright (C) 2021-2022 Nils Asmussen, Barkhausen Institut
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

//! Contains the interface between applications and TileMux

use crate::arch::{TMABIOps, TMABI};
use crate::errors::{Code, Error};
use crate::goff;
use crate::kif;
use crate::tcu::{EpId, INVALID_EP};
use crate::time::TimeDuration;

pub type IRQId = u32;

pub const INVALID_IRQ: IRQId = !0;

int_enum! {
    /// The operations TileMux supports
    pub struct Operation : usize {
        /// Wait for an event, optionally with timeout
        const WAIT          = 0x0;
        /// Exit the application
        const EXIT          = 0x1;
        /// Switch to the next ready activity
        const YIELD         = 0x2;
        /// Map local physical memory (IO memory)
        const MAP           = 0x3;
        /// Register for a given interrupt
        const REG_IRQ       = 0x4;
        /// For TCU TLB misses
        const TRANSL_FAULT  = 0x5;
        /// Flush and invalidate cache
        const FLUSH_INV     = 0x6;
        /// Initializes thread-local storage (x86 only)
        const INIT_TLS      = 0x7;
        /// Noop operation for testing purposes
        const NOOP          = 0x8;
    }
}

pub(crate) fn get_result(res: usize) -> Result<(), Error> {
    Result::from(Code::from(res as u32))
}

#[inline(always)]
pub fn wait(
    ep: Option<EpId>,
    irq: Option<IRQId>,
    duration: Option<TimeDuration>,
) -> Result<(), Error> {
    TMABI::call3(
        Operation::WAIT,
        ep.unwrap_or(INVALID_EP) as usize,
        irq.unwrap_or(INVALID_IRQ) as usize,
        match duration {
            Some(d) => d.as_nanos() as usize,
            None => usize::MAX,
        },
    )
    .map(|_| ())
}

pub fn exit(code: Code) -> ! {
    TMABI::call1(Operation::EXIT, code as usize).ok();
    unreachable!();
}

pub fn map(virt: usize, phys: goff, pages: usize, access: kif::Perm) -> Result<(), Error> {
    TMABI::call4(
        Operation::MAP,
        virt,
        phys as usize,
        pages,
        access.bits() as usize,
    )
}

pub fn reg_irq(irq: IRQId) -> Result<(), Error> {
    TMABI::call1(Operation::REG_IRQ, irq as usize)
}

pub fn flush_invalidate() -> Result<(), Error> {
    TMABI::call1(Operation::FLUSH_INV, 0)
}

#[inline(always)]
pub fn switch_activity() -> Result<(), Error> {
    TMABI::call1(Operation::YIELD, 0)
}

#[inline(always)]
pub fn noop() -> Result<(), Error> {
    TMABI::call1(Operation::NOOP, 0)
}
