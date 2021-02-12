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

use base::backtrace;
use base::envdata;
use base::int_enum;
use base::libc;
use base::{set_csr_bits, write_csr};
use base::tcu;
use core::fmt;

pub const ISR_COUNT: usize = 32;
pub const TCU_ISR: usize = Vector::SUPER_EXT_IRQ.val;

#[derive(Default)]
// see comment in ARM code
#[repr(C, align(8))]
pub struct State {
    // general purpose registers
    pub r: [usize; 31],
    pub cause: usize,
    pub epc: usize,
    pub status: usize,
}

impl State {
    pub fn base_pointer(&self) -> usize {
        self.r[7]
    }

    pub fn came_from_user(&self) -> bool {
        ((self.status >> 8) & 1) == 0
    }
}

int_enum! {
    pub struct Vector : usize {
        // exceptions
        const INSTR_MISALIGNED = 0;
        const INSTR_ACC_FAULT = 1;
        const ILLEGAL_INSTR = 2;
        const BREAKPOINT = 3;
        const LOAD_MISALIGNED = 4;
        const LOAD_ACC_FAULT = 5;
        const STORE_MISALIGNED = 6;
        const STORE_ACC_FAULT = 7;
        const ENV_UCALL = 8;
        const ENV_SCALL = 9;
        const ENV_MCALL = 11;
        const INSTR_PAGEFAULT = 12;
        const LOAD_PAGEFAULT = 13;
        const STORE_PAGEFAULT = 15;

        // interrupts
        const USER_SW_IRQ = 16;
        const SUPER_SW_IRQ = 17;
        const MACH_SW_IRQ = 19;
        const USER_TIMER_IRQ = 20;
        const SUPER_TIMER_IRQ = 21;
        const MACH_TIMER_IRQ = 23;
        const USER_EXT_IRQ = 24;
        const SUPER_EXT_IRQ = 25;
        const MACH_EXT_IRQ = 27;
    }
}

impl fmt::Debug for State {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let vec = if (self.cause & 0x8000_0000_0000_0000) != 0 {
            16 + (self.cause & 0xF)
        }
        else {
            self.cause & 0xF
        };

        writeln!(fmt, "  vec: {:#x} ({})", vec, Vector::from(vec))?;
        for (idx, r) in { self.r }.iter().enumerate() {
            writeln!(fmt, "  r[{:02}]:  {:#x}", idx + 1, r)?;
        }
        writeln!(fmt, "  cause:  {:#x}", { self.cause })?;
        writeln!(fmt, "  epc:    {:#x}", { self.epc })?;
        writeln!(fmt, "  status: {:#x}", { self.status })?;

        writeln!(fmt, "\nUser backtrace:")?;
        let mut bt = [0usize; 16];
        let bt_len = backtrace::collect_for(self.base_pointer(), &mut bt);
        for addr in bt.iter().take(bt_len) {
            writeln!(fmt, "  {:#x}", addr)?;
        }
        Ok(())
    }
}

mod plic {
    pub const TCU_ID: u32 = 1;

    const MMIO_PRIORITY: *mut u32 = 0x0C00_0000 as *mut u32;
    const MMIO_ENABLE: *mut u32 = 0x0C00_2000 as *mut u32;
    const MMIO_THRESHOLD: *mut u32 = 0x0C20_0000 as *mut u32;
    const MMIO_CLAIM: *mut u32 = 0x0C20_0004 as *mut u32;

    pub fn fetch_and_ack() {
        unsafe {
            let next = MMIO_CLAIM.read_volatile();
            assert!(next != 0);
            MMIO_CLAIM.write_volatile(next);
        }
    }

    pub fn enable(id: u32) {
        unsafe {
            let val = MMIO_ENABLE.read_volatile();
            MMIO_ENABLE.write_volatile(val | (1 << id));
        }
    }

    pub fn set_priority(id: u32, prio: u8) {
        unsafe {
            MMIO_PRIORITY
                .add(id as usize)
                .write_volatile(prio as u32 & 0x7);
        }
    }

    pub fn set_threshold(threshold: u8) {
        unsafe {
            MMIO_THRESHOLD.write_volatile(threshold as u32 & 0x7);
        }
    }
}

extern "C" {
    fn isr_setup(stack: usize);
}

#[no_mangle]
pub extern "C" fn isr_handler(state: &mut State) -> *mut libc::c_void {
    let vec = if (state.cause & 0x8000_0000_0000_0000) != 0 {
        16 + (state.cause & 0xF)
    }
    else {
        state.cause & 0xF
    };

    // don't repeat the ECALL instruction
    if vec >= 8 && vec <= 10 {
        state.epc += 4;
    }

    crate::ISRS[vec](state)
}

pub fn init(state: &mut State) {
    if envdata::get().platform == envdata::Platform::HW.val {
        plic::set_threshold(0);
        plic::enable(plic::TCU_ID);
        plic::set_priority(plic::TCU_ID, 1);
    }

    unsafe {
        let state_top = (state as *mut State).offset(1) as usize;
        isr_setup(state_top)
    };
}

pub fn set_entry_sp(sp: usize) {
    write_csr!("sscratch", sp);
}

pub fn enable_irqs() {
    // set SIE to 1
    set_csr_bits!("sstatus", 1 << 1);
}

pub fn acknowledge_irq(irq: tcu::IRQ) {
    if envdata::get().platform == envdata::Platform::HW.val {
        // TODO: temporary (add to spec and make gem5 behave the same)
        let tcu_set_irq_addr = 0xF000_3030 as *mut u64;
        unsafe {
            tcu_set_irq_addr.add(irq.val as usize).write_volatile(0);
        }

        plic::fetch_and_ack();
    }
    else {
        tcu::TCU::clear_irq(irq);
    }
}
