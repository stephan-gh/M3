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

use base::cpu;
use core::fmt;

pub const ISR_COUNT: usize = 66;
pub const TCU_IRQ: usize = 64;

pub const DPL_USER: u64 = 3;
pub const DPL_KERNEL: u64 = 0;

#[derive(Default)]
// see comment in ARM code
#[repr(C, align(16))]
pub struct State {
    // general purpose registers
    pub r: [usize; 15],
    // interrupt-number
    pub irq: usize,
    // error-code (for exceptions); default = 0
    pub error: usize,
    // pushed by the CPU
    pub rip: usize,
    pub cs: usize,
    pub rflags: usize,
    pub rsp: usize,
    pub ss: usize,
}

impl State {
    pub fn came_from_user(&self) -> bool {
        (self.cs & DPL_USER as usize) == DPL_USER as usize
    }
}

fn vec_name(vec: usize) -> &'static str {
    match vec {
        0x00 => "Divide by zero",
        0x01 => "Single step",
        0x02 => "Non maskable",
        0x03 => "Breakpoint",
        0x04 => "Overflow",
        0x05 => "Bounds check",
        0x06 => "Invalid opcode",
        0x07 => "Co-proc. n/a",
        0x08 => "Double fault",
        0x09 => "Co-proc seg. overrun",
        0x0A => "Invalid TSS",
        0x0B => "Segment not present",
        0x0C => "Stack exception",
        0x0D => "Gen. prot. fault",
        0x0E => "Page fault",
        0x10 => "Co-processor error",
        _ => "<unknown>",
    }
}

impl fmt::Debug for State {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let cr2 = cpu::read_cr2();
        writeln!(fmt, "State @ {:#x}", self as *const State as usize)?;
        writeln!(fmt, "  vec: {:#x} ({})", { self.irq }, vec_name(self.irq))?;
        writeln!(fmt, "  cr2:    {:#x}", cr2)?;
        writeln!(fmt, "  error:  {:#x}", { self.error })?;
        writeln!(fmt, "  rip:    {:#x}", { self.rip })?;
        writeln!(fmt, "  rflags: {:#x}", { self.rflags })?;
        writeln!(fmt, "  rsp:    {:#x}", { self.rsp })?;
        writeln!(fmt, "  cs:     {:#x}", { self.cs })?;
        writeln!(fmt, "  ss:     {:#x}", { self.ss })?;
        for (idx, r) in { self.r }.iter().enumerate() {
            writeln!(fmt, "  r[{:02}]:  {:#x}", idx, r)?;
        }
        Ok(())
    }
}
