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
use base::errors::Error;
use base::kif::{pemux, PageFlags};
use base::libc;
use core::fmt;
use core::mem::MaybeUninit;

use vma;
use vpe;

type IsrFunc = extern "C" fn(state: &mut State) -> *mut libc::c_void;

extern "C" {
    fn isr_init(stack: usize);
    fn isr_reg(idx: usize, func: IsrFunc);

    fn save_fpu(state: &mut FPUState);
    fn restore_fpu(state: &FPUState);
}

int_enum! {
    struct Vector : usize {
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

#[derive(Default)]
// see comment in ARM code
#[repr(C, align(8))]
pub struct State {
    // general purpose registers
    pub r: [usize; 31],
    pub cause: usize,
    pub sepc: usize,
    pub sstatus: usize,
}

#[repr(C, align(8))]
pub struct FPUState {
    r: [MaybeUninit<usize>; 32],
    fcsr: MaybeUninit<usize>,
    init: bool,
}

impl Default for FPUState {
    fn default() -> Self {
        Self {
            // we init that lazy on the first use of the FPU
            r: unsafe { MaybeUninit::uninit().assume_init() },
            fcsr: unsafe { MaybeUninit::uninit().assume_init() },
            init: false,
        }
    }
}

int_enum! {
    struct FSMode : usize {
        const OFF = 0;
        const INITIAL = 1;
        const CLEAN = 2;
        const DIRTY = 3;
    }
}

pub const PEXC_ARG0: usize = 9; // a0 = x10
pub const PEXC_ARG1: usize = 10; // a1 = x11
pub const PEXC_ARG2: usize = 11; // a2 = x12

static FPU_OWNER: StaticCell<vpe::Id> = StaticCell::new(pemux::VPE_ID);

impl fmt::Debug for State {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let vec = if (self.cause & 0x8000_0000) != 0 {
            16 + (self.cause & 0xF)
        }
        else {
            self.cause & 0xF
        };

        writeln!(fmt, "State @ {:#x}", self as *const State as usize)?;
        writeln!(fmt, "  vec: {:#x} ({})", vec, Vector::from(vec))?;
        for (idx, r) in { self.r }.iter().enumerate() {
            writeln!(fmt, "  r[{:02}]:  {:#x}", idx + 1, r)?;
        }
        writeln!(fmt, "  cause:  {:#x}", { self.cause })?;
        writeln!(fmt, "  sepc:   {:#x}", { self.sepc })?;
        writeln!(fmt, "  status: {:#x}", { self.sstatus })?;
        Ok(())
    }
}

impl State {
    pub fn came_from_user(&self) -> bool {
        ((self.sstatus >> 8) & 1) == 0
    }

    pub fn init(&mut self, entry: usize, sp: usize) {
        self.r[9] = 0xDEAD_BEEF; // a0; don't set the stackpointer in crt0
        self.sepc = entry;
        self.r[1] = sp;
        self.sstatus = read_csr!("sstatus");
        self.sstatus &= !(1 << 8); // user mode
        self.sstatus |= 1 << 5; // interrupts enabled
        self.sstatus = set_fpu_mode(self.sstatus, FSMode::OFF);
    }
}

fn get_fpu_mode(sstatus: usize) -> FSMode {
    FSMode::from((sstatus >> 13) & 0x3)
}

fn set_fpu_mode(mut sstatus: usize, mode: FSMode) -> usize {
    sstatus &= !(0x3 << 13);
    sstatus | (mode.val << 13)
}

pub fn set_entry_sp(sp: usize) {
    write_csr!("sscratch", sp);
}

pub fn init(stack: usize) {
    unsafe {
        isr_init(stack);
        for i in 0..=31 {
            match Vector::from(i) {
                Vector::ILLEGAL_INSTR => isr_reg(i, crate::fpu_ex),
                Vector::ENV_UCALL => isr_reg(i, crate::pexcall),
                Vector::INSTR_PAGEFAULT => isr_reg(i, crate::mmu_pf),
                Vector::LOAD_PAGEFAULT => isr_reg(i, crate::mmu_pf),
                Vector::STORE_PAGEFAULT => isr_reg(i, crate::mmu_pf),
                Vector::SUPER_EXT_IRQ => isr_reg(i, crate::tcu_irq),
                Vector::SUPER_TIMER_IRQ => isr_reg(i, crate::timer_irq),
                _ => isr_reg(i, crate::unexpected_irq),
            }
        }
    }
}

pub fn forget_fpu(vpe_id: vpe::Id) {
    if *FPU_OWNER == vpe_id {
        FPU_OWNER.set(pemux::VPE_ID);
    }
}

pub fn disable_fpu() {
    let cur = vpe::cur();
    if cur.id() != *FPU_OWNER {
        cur.user_state().sstatus = set_fpu_mode(cur.user_state().sstatus, FSMode::OFF);
    }
}

pub fn handle_fpu_ex(state: &mut State) {
    let cur = vpe::cur();

    // if the FPU is enabled and we receive an illegal instruction exception, kill VPE
    if get_fpu_mode(state.sstatus) != FSMode::OFF {
        log!(crate::LOG_ERR, "Illegal instruction with {:?}", state);
        vpe::remove_cur(1);
        return;
    }

    // enable FPU
    state.sstatus = set_fpu_mode(state.sstatus, FSMode::CLEAN);

    let old_id = *FPU_OWNER & 0xFFFF;
    if old_id != cur.id() {
        // enable FPU so that we can save/restore the FPU registers
        write_csr!("sstatus", set_fpu_mode(read_csr!("sstatus"), FSMode::CLEAN));

        // need to save old state?
        if old_id != pemux::VPE_ID {
            let old_vpe = vpe::get_mut(old_id).unwrap();
            unsafe { save_fpu(old_vpe.fpu_state()) };
        }

        // restore new state
        let fpu_state = cur.fpu_state();
        if fpu_state.init {
            unsafe { restore_fpu(fpu_state) };
        }
        else {
            unsafe { libc::memset(fpu_state as *mut _ as *mut libc::c_void, 0, 8 * 33) };
            fpu_state.init = true;
        }

        // we are owner now
        FPU_OWNER.set(cur.id());
    }
}

pub fn handle_mmu_pf(state: &mut State) -> Result<(), Error> {
    let virt = read_csr!("stval");

    let perm = match Vector::from(state.cause & 0x1F) {
        Vector::INSTR_PAGEFAULT => PageFlags::R | PageFlags::X,
        Vector::LOAD_PAGEFAULT => PageFlags::R,
        Vector::STORE_PAGEFAULT => PageFlags::R | PageFlags::W,
        _ => unreachable!(),
    };

    vma::handle_pf(state, virt, perm, state.sepc)
}
