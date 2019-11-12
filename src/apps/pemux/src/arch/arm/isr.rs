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

use base::cell::StaticCell;
use base::libc;
use core::fmt;
use isr;

use vpe;

type IsrFunc = extern "C" fn(state: &mut isr::State) -> *mut libc::c_void;

extern "C" {
    fn isr_init();
    fn isr_reg(idx: usize, func: IsrFunc);
    fn isr_enable();

    static idle_stack: libc::c_void;
}

fn vec_name(vec: usize) -> &'static str {
    match vec {
        0 => "Reset",
        1 => "UndefInstr",
        2 => "SWI",
        3 => "PrefetchAbort",
        4 => "DataAbort",
        5 => "Reserved",
        6 => "IRQ",
        _ => "FIQ",
    }
}

#[repr(C, packed)]
pub struct State {
    pub sp: usize,
    pub lr: usize,
    pub vec: usize,
    pub r: [usize; 13], // r0 .. r12
    pub klr: usize,
    pub pc: usize,
    pub cpsr: usize,
}

pub const PEXC_ARG0: usize = 0; // r0
pub const PEXC_ARG1: usize = 1; // r1

impl fmt::Debug for State {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        writeln!(fmt, "State @ {:#x}", self as *const State as usize)?;
        writeln!(fmt, "  lr:     {:#x}", { self.lr })?;
        writeln!(fmt, "  sp:     {:#x}", { self.sp })?;
        writeln!(fmt, "  vec: {:#x} ({})", { self.vec }, vec_name(self.vec))?;
        writeln!(fmt, "  klr:    {:#x}", { self.klr })?;
        for (idx, r) in { self.r }.iter().enumerate() {
            writeln!(fmt, "  r[{:02}]:  {:#x}", idx, r)?;
        }
        writeln!(fmt, "  pc:     {:#x}", { self.pc })?;
        writeln!(fmt, "  cpsr:   {:#x}", { self.cpsr })?;
        Ok(())
    }
}

static STOPPED: StaticCell<bool> = StaticCell::new(false);

impl State {
    pub fn came_from_user(&self) -> bool {
        (self.cpsr & 0x0F) == 0x0
    }

    pub fn nested(&self) -> bool {
        !self.came_from_user()
    }

    pub fn init(&mut self, entry: usize, sp: usize) {
        self.r[1] = 0xDEADBEEF; // don't set the stackpointer in crt0
        self.pc = entry;
        self.sp = sp;
        self.cpsr = 0x10; // user mode
        self.lr = 0;
    }

    pub fn stop(&mut self) {
        if self.nested() {
            *STOPPED.get_mut() = true;
        }
        else {
            self.pc = crate::sleep as *const fn() as usize;
            self.sp = unsafe { &idle_stack as *const libc::c_void as usize };

            vpe::remove();

            *STOPPED.get_mut() = false;
        }
    }

    pub fn finalize(&mut self) -> *mut libc::c_void {
        if *STOPPED {
            self.stop();
        }
        self as *mut Self as *mut libc::c_void
    }
}

pub fn enable_ints() -> bool {
    // not necessary, because PE-type C is not supported anyway
    false
}

pub fn restore_ints(_prev: bool) {
}

pub fn init() {
    unsafe {
        isr_init();
        for i in 0..=7 {
            match i {
                2 => isr_reg(i, crate::pexcall),
                6 => isr_reg(i, crate::dtu_irq),
                i => isr_reg(i, crate::unexpected_irq),
            }
        }
        isr_enable();
    }
}
