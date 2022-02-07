/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

use base::errors::Error;
use base::kif::PageFlags;

use crate::activities;
use crate::vma;

pub type State = isr::State;

pub fn init_state(state: &mut State, entry: usize, sp: usize) {
    state.r[1] = 0xDEAD_BEEF; // don't set the stackpointer in crt0
    state.pc = entry;
    state.sp = sp;
    state.cpsr = 0x10; // user mode
    state.lr = 0;
}

pub fn init(state: &mut State) {
    isr::init(state);
    for i in 0..=7 {
        match isr::Vector::from(i) {
            isr::Vector::SWI => isr::reg(i, crate::tmcall),
            isr::Vector::PREFETCH_ABORT => isr::reg(i, crate::mmu_pf),
            isr::Vector::DATA_ABORT => isr::reg(i, crate::mmu_pf),
            isr::Vector::IRQ => isr::reg(i, crate::ext_irq),
            _ => isr::reg(i, crate::unexpected_irq),
        }
    }
}

pub fn forget_fpu(_act_id: activities::Id) {
    // no FPU support
}

pub fn disable_fpu() {
    // no FPU support
}

pub fn handle_mmu_pf(state: &mut State) -> Result<(), Error> {
    let (virt, perm) = if state.vec == isr::Vector::DATA_ABORT.val {
        let dfar: usize;
        let dfsr: usize;
        unsafe {
            asm!(
                "mrc p15, 0, {0}, c6, c0, 0",
                "mrc p15, 0, {1}, c5, c0, 0",
                out(reg) dfar,
                out(reg) dfsr,
                options(nostack, nomem),
            );
        }
        (
            dfar,
            if dfsr & 0x800 != 0 {
                PageFlags::RW
            }
            else {
                PageFlags::R
            },
        )
    }
    else {
        let ifar: usize;
        unsafe {
            asm!(
                "mrc p15, 0, {0}, c6, c0, 2",
                out(reg) ifar,
                options(nostack, nomem),
            );
        }
        (ifar, PageFlags::RX)
    };

    vma::handle_pf(state, virt, perm, state.pc)
}
