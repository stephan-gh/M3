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
use base::cpu;
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
    fn isr_set_sp(sp: usize);
}

pub const DPL_USER: u64 = 3;

pub const SEG_UCODE: u64 = 3;
pub const SEG_UDATA: u64 = 4;

pub const PEXC_ARG0: usize = 14; // rax
pub const PEXC_ARG1: usize = 12; // rcx
pub const PEXC_ARG2: usize = 11; // rdx

const CR0_TASK_SWITCHED: usize = 1 << 3;

static FPU_OWNER: StaticCell<vpe::Id> = StaticCell::new(pemux::VPE_ID);

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

#[repr(C, packed)]
pub struct FPUState {
    data: [MaybeUninit<u8>; 512],
    init: bool,
}

impl Default for FPUState {
    fn default() -> Self {
        Self {
            // we init that lazy on the first use of the FPU
            #[allow(clippy::uninit_assumed_init)]
            data: unsafe { MaybeUninit::uninit().assume_init() },
            init: false,
        }
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
        writeln!(fmt, "State @ {:#x}", self as *const State as usize)?;
        writeln!(fmt, "  vec: {:#x} ({})", { self.irq }, vec_name(self.irq))?;
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

impl State {
    pub fn came_from_user(&self) -> bool {
        (self.cs & DPL_USER as usize) == DPL_USER as usize
    }

    pub fn init(&mut self, entry: usize, sp: usize) {
        self.rip = entry;
        self.rsp = sp;
        self.r[8] = 0; // rbp
        self.r[14] = 0xDEAD_BEEF; // set rax to tell crt0 that we've set the SP

        self.rflags = 0x200; // enable interrupts

        // run in user mode
        self.cs = ((SEG_UCODE << 3) | DPL_USER) as usize;
        self.ss = ((SEG_UDATA << 3) | DPL_USER) as usize;
    }
}

pub fn set_entry_sp(sp: usize) {
    unsafe { isr_set_sp(sp) };
}

pub fn init(stack: usize) {
    unsafe {
        isr_init(stack);
        for i in 0..=65 {
            match i {
                7 => isr_reg(i, crate::fpu_ex),
                14 => isr_reg(i, crate::mmu_pf),
                63 => isr_reg(i, crate::pexcall),
                64 => isr_reg(i, crate::tcu_irq),
                65 => isr_reg(i, crate::timer_irq),
                i => isr_reg(i, crate::unexpected_irq),
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
    if vpe::cur().id() != *FPU_OWNER {
        cpu::write_cr0(cpu::read_cr0() | CR0_TASK_SWITCHED);
    }
}

pub fn handle_fpu_ex(_state: &mut State) {
    let cur = vpe::cur();

    cpu::write_cr0(cpu::read_cr0() & !CR0_TASK_SWITCHED);

    let old_id = *FPU_OWNER & 0xFFFF;
    if old_id != cur.id() {
        // need to save old state?
        if old_id != pemux::VPE_ID {
            let old_vpe = vpe::get_mut(old_id).unwrap();
            let fpu_state = old_vpe.fpu_state();
            unsafe { llvm_asm!("fxsave ($0)" : : "r"(&fpu_state.data)) };
        }

        // restore new state
        let fpu_state = cur.fpu_state();
        if fpu_state.init {
            unsafe { llvm_asm!("fxrstor ($0)" : : "r"(&fpu_state.data)) };
        }
        else {
            unsafe { llvm_asm!("fninit") };
            fpu_state.init = true;
        }

        // we are owner now
        FPU_OWNER.set(cur.id());
    }
}

pub fn handle_mmu_pf(state: &mut State) -> Result<(), Error> {
    let cr2 = cpu::read_cr2();

    let perm =
        paging::MMUFlags::from_bits_truncate(state.error as paging::MMUPTE & PageFlags::RW.bits());
    // the access is implicitly no-exec
    let perm = paging::to_page_flags(0, perm | paging::MMUFlags::NX);

    vma::handle_pf(state, cr2, perm, state.rip)
}
