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
use base::kif::PageFlags;
use base::libc;
use core::fmt;
use core::ptr;

use vma;

type IsrFunc = extern "C" fn(state: &mut State) -> *mut libc::c_void;

extern "C" {
    fn isr_init();
    fn isr_reg(idx: usize, func: IsrFunc);
    fn isr_enable();

    static idle_stack: libc::c_void;
}

int_enum! {
    struct Vector : usize {
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
        writeln!(
            fmt,
            "  vec: {:#x} ({})",
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
    let enabled: u32;
    unsafe {
        asm!(
            "mrs $0, cpsr; cpsie if; and $0, $0, #0xc0;"
            : "=r"(enabled) : : "memory"
        );
    }
    enabled != 0
}

pub fn restore_ints(prev: bool) {
    let flags = match prev {
        true => 0xc0,
        false => 0,
    };
    unsafe {
        asm!(
            "mrs r1, cpsr; bic r1, r1, $0; msr cpsr_c, r1"
            : : "r"(flags) : "r1", "memory"
        );
    }
}

pub fn is_stopped() -> bool {
    // use volatile because STOPPED may have changed via a nested IRQ
    unsafe { ptr::read_volatile(STOPPED.get_mut()) }
}

pub fn init() {
    unsafe {
        isr_init();
        for i in 0..=7 {
            match Vector::from(i) {
                Vector::SWI => isr_reg(i, crate::pexcall),
                Vector::PREFETCH_ABORT => isr_reg(i, crate::mmu_pf),
                Vector::DATA_ABORT => isr_reg(i, crate::mmu_pf),
                Vector::IRQ => isr_reg(i, crate::dtu_irq),
                _ => isr_reg(i, crate::unexpected_irq),
            }
        }
        isr_enable();
    }
}

pub fn handle_mmu_pf(state: &mut State) {
    let (virt, perm) = if state.vec == Vector::DATA_ABORT.val {
        let dfar: usize;
        let dfsr: usize;
        unsafe {
            asm!("mrc p15, 0, $0, c6, c0, 0; mrc p15, 0, $1, c5, c0, 0" : "=r"(dfar), "=r"(dfsr));
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
            asm!("mrc p15, 0, $0, c6, c0, 2" : "=r"(ifar));
        }
        (ifar, PageFlags::RX)
    };

    vma::handle_pf(state.came_from_user(), virt, perm, state.pc);
}
