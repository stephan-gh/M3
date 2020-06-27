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
use core::mem::MaybeUninit;

use vma;
use vpe;

pub type State = isr::State;

#[repr(C, align(8))]
pub struct FPUState {
    r: [MaybeUninit<usize>; 32],
    fcsr: usize,
    init: bool,
}

impl Default for FPUState {
    fn default() -> Self {
        Self {
            // we init that lazy on the first use of the FPU
            r: unsafe { MaybeUninit::uninit().assume_init() },
            fcsr: 0,
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

macro_rules! ldst_fpu_regs {
    ($ins:tt, $base:expr, $($no:tt)*) => {
        $(
            llvm_asm!(concat!($ins, " f", $no, ", 8*", $no, "($0)") : : "r"($base) : : "volatile");
        )*
    };
}

fn save_fpu(state: &mut FPUState) {
    unsafe {
        ldst_fpu_regs!(
            "fsd",
            state as *mut _ as usize,
            0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31
        );
    }
    state.fcsr = read_csr!("fcsr");
}

fn restore_fpu(state: &FPUState) {
    unsafe {
        ldst_fpu_regs!(
            "fld",
            state as *const _ as usize,
            0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31
        );
    }
    write_csr!("fcsr", state.fcsr);
}

fn get_fpu_mode(sstatus: usize) -> FSMode {
    FSMode::from((sstatus >> 13) & 0x3)
}

fn set_fpu_mode(mut sstatus: usize, mode: FSMode) -> usize {
    sstatus &= !(0x3 << 13);
    sstatus | (mode.val << 13)
}

pub fn init(stack: usize) {
    isr::init(stack);
    for i in 0..=31 {
        match isr::Vector::from(i) {
            isr::Vector::ILLEGAL_INSTR => isr::reg(i, crate::fpu_ex),
            isr::Vector::ENV_UCALL => isr::reg(i, crate::pexcall),
            isr::Vector::INSTR_PAGEFAULT => isr::reg(i, crate::mmu_pf),
            isr::Vector::LOAD_PAGEFAULT => isr::reg(i, crate::mmu_pf),
            isr::Vector::STORE_PAGEFAULT => isr::reg(i, crate::mmu_pf),
            isr::Vector::SUPER_EXT_IRQ => isr::reg(i, crate::tcu_irq),
            isr::Vector::SUPER_TIMER_IRQ => isr::reg(i, crate::timer_irq),
            _ => isr::reg(i, crate::unexpected_irq),
        }
    }
}

pub fn init_state(state: &mut State, entry: usize, sp: usize) {
    state.r[9] = 0xDEAD_BEEF; // a0; don't set the stackpointer in crt0
    state.sepc = entry;
    state.r[1] = sp;
    state.sstatus = read_csr!("sstatus");
    state.sstatus &= !(1 << 8); // user mode
    state.sstatus |= 1 << 5; // interrupts enabled
    state.sstatus = set_fpu_mode(state.sstatus, FSMode::OFF);
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
            save_fpu(old_vpe.fpu_state());
        }

        // restore new state
        let fpu_state = cur.fpu_state();
        if fpu_state.init {
            restore_fpu(fpu_state);
        }
        else {
            unsafe {
                libc::memset(fpu_state as *mut _ as *mut libc::c_void, 0, 8 * 33)
            };
            fpu_state.init = true;
        }

        // we are owner now
        FPU_OWNER.set(cur.id());
    }
}

pub fn handle_mmu_pf(state: &mut State) -> Result<(), Error> {
    let virt = read_csr!("stval");

    let perm = match isr::Vector::from(state.cause & 0x1F) {
        isr::Vector::INSTR_PAGEFAULT => PageFlags::R | PageFlags::X,
        isr::Vector::LOAD_PAGEFAULT => PageFlags::R,
        isr::Vector::STORE_PAGEFAULT => PageFlags::R | PageFlags::W,
        _ => unreachable!(),
    };

    vma::handle_pf(state, virt, perm, state.sepc)
}
