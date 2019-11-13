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

use core::fmt;

pub const ISR_COUNT: usize = 8;
pub const TCU_IRQ: usize = Vector::IRQ.val;

#[derive(Default)]
// for some reason, we need to specify the alignment here. actually, this struct needs to be packed,
// but unfortunately, we cannot specify both packed and align. but without packed seems to be fine,
// because there are no holes between the fields.
#[repr(C, align(4))]
pub struct State {
    pub sp: usize,
    pub lr: usize,
    pub vec: usize,
    pub r: [usize; 13], // r0 .. r12
    pub klr: usize,
    pub pc: usize,
    pub cpsr: usize,
}

impl State {
    #[allow(clippy::verbose_bit_mask)]
    pub fn came_from_user(&self) -> bool {
        (self.cpsr & 0x0F) == 0x0
    }
}

int_enum! {
    pub struct Vector : usize {
        const RESET = 0;
        const UNDEF_INSTR = 1;
        const SWI = 2;
        const PREFETCH_ABORT = 3;
        const DATA_ABORT = 4;
        const RESERVED = 5;
        const IRQ = 6;
        const FIQ = 7;
    }
}

impl fmt::Debug for State {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        writeln!(fmt, "State @ {:#x}", self as *const State as usize)?;
        writeln!(fmt, "  lr:     {:#x}", { self.lr })?;
        writeln!(fmt, "  sp:     {:#x}", { self.sp })?;
        writeln!(
            fmt,
            "  vec:    {:#x} ({})",
            { self.vec },
            Vector::from(self.vec)
        )?;
        writeln!(fmt, "  klr:    {:#x}", { self.klr })?;
        for (idx, r) in { self.r }.iter().enumerate() {
            writeln!(fmt, "  r[{:02}]:  {:#x}", idx, r)?;
        }
        writeln!(fmt, "  pc:     {:#x}", { self.pc })?;
        writeln!(fmt, "  cpsr:   {:#x}", { self.cpsr })?;
        Ok(())
    }
}
