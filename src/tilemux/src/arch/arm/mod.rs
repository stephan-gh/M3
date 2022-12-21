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

use crate::activities;

pub type State = isr::State;

pub fn init_state(state: &mut State, entry: usize, sp: usize) {
    state.r[1] = 0xDEAD_BEEF; // don't set the stackpointer in crt0
    state.pc = entry;
    state.sp = sp;
    state.cpsr = 0x10; // user mode
    state.lr = 0;
}

pub fn forget_fpu(_act_id: activities::Id) {
    // no FPU support
}

pub fn disable_fpu() {
    // no FPU support
}
