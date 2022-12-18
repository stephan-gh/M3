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

use base::cell::StaticCell;
use base::errors::Error;
use base::kif::{tilemux, PageFlags};
use base::mem::MaybeUninit;
use base::{read_csr, write_csr};

use core::arch::asm;

use crate::activities;
use crate::vma;

pub type State = isr::State;

const CR0_TASK_SWITCHED: usize = 1 << 3;

static FPU_OWNER: StaticCell<activities::Id> = StaticCell::new(tilemux::ACT_ID);

#[repr(C, packed)]
pub struct FPUState {
    data: [MaybeUninit<u8>; 512],
    init: bool,
}

impl Default for FPUState {
    fn default() -> Self {
        Self {
            // we init that lazy on the first use of the FPU
            data: unsafe { MaybeUninit::uninit().assume_init() },
            init: false,
        }
    }
}

pub fn init(state: &mut State) {
    isr::init(state);
    for i in 0..=65 {
        match i {
            7 => isr::reg(i, crate::fpu_ex),
            14 => isr::reg(i, crate::mmu_pf),
            63 => isr::reg(i, crate::tmcall),
            64 => isr::reg(i, crate::ext_irq),
            65 => isr::reg(i, crate::ext_irq),
            i => isr::reg(i, crate::unexpected_irq),
        }
    }
}

pub fn init_state(state: &mut State, entry: usize, sp: usize) {
    state.rip = entry;
    state.rsp = sp;
    state.r[8] = 0; // rbp
    state.r[14] = 0xDEAD_BEEF; // set rax to tell crt0 that we've set the SP

    state.rflags = 0x200; // enable interrupts

    // run in user mode
    state.cs = ((isr::Segment::UCODE.val << 3) | isr::DPL::USER.val) as usize;
    state.ss = ((isr::Segment::UDATA.val << 3) | isr::DPL::USER.val) as usize;
}

pub fn forget_fpu(act_id: activities::Id) {
    if FPU_OWNER.get() == act_id {
        FPU_OWNER.set(tilemux::ACT_ID);
    }
}

pub fn disable_fpu() {
    if activities::cur().id() != FPU_OWNER.get() {
        write_csr!("cr0", read_csr!("cr0") | CR0_TASK_SWITCHED);
    }
}

pub fn handle_fpu_ex(_state: &mut State) {
    let mut cur = activities::cur();

    write_csr!("cr0", read_csr!("cr0") & !CR0_TASK_SWITCHED);

    let old_id = FPU_OWNER.get() & 0xFFFF;
    if old_id != cur.id() {
        // need to save old state?
        if old_id != tilemux::ACT_ID {
            let mut old_act = activities::get_mut(old_id).unwrap();
            let fpu_state = old_act.fpu_state();
            unsafe {
                asm!(
                    "fxsave [{0}]",
                    in(reg) &fpu_state.data,
                    options(nostack),
                )
            };
        }

        // restore new state
        let fpu_state = cur.fpu_state();
        if fpu_state.init {
            unsafe {
                asm!(
                    "fxrstor [{0}]",
                    in(reg) &fpu_state.data,
                    options(nostack),
                )
            };
        }
        else {
            unsafe { asm!("fninit") };
            fpu_state.init = true;
        }

        // we are owner now
        FPU_OWNER.set(cur.id());
    }
}

pub fn handle_mmu_pf(state: &mut State) -> Result<(), Error> {
    let cr2 = read_csr!("cr2");

    let perm =
        paging::MMUFlags::from_bits_truncate(state.error as paging::MMUPTE & PageFlags::RW.bits());
    // the access is implicitly no-exec
    let perm = paging::to_page_flags(0, perm | paging::MMUFlags::NX);

    vma::handle_pf(state, cr2, perm, state.rip)
}
