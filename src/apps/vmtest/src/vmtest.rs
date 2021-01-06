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

#![feature(llvm_asm)]
#![no_std]

extern crate heap;

mod paging;

use base::cell::LazyStaticCell;
use base::cfg;
use base::io;
use base::libc;
use base::kif::PageFlags;
use base::log;
use base::machine;
use base::read_csr;

static LOG_DEF: bool = true;

#[no_mangle]
pub extern "C" fn abort() {
    exit(1);
}

#[no_mangle]
pub extern "C" fn exit(_code: i32) {
    machine::shutdown();
}

pub extern "C" fn mmu_pf(state: &mut isr::State) -> *mut libc::c_void {
    let virt = read_csr!("stval");

    let perm = match isr::Vector::from(state.cause & 0x1F) {
        isr::Vector::INSTR_PAGEFAULT => PageFlags::R | PageFlags::X,
        isr::Vector::LOAD_PAGEFAULT => PageFlags::R,
        isr::Vector::STORE_PAGEFAULT => PageFlags::R | PageFlags::W,
        _ => unreachable!(),
    };

    panic!("Pagefault for address={:#x}, perm={:?} with {:?}", virt, perm, state);
}

static STATE: LazyStaticCell<isr::State> = LazyStaticCell::default();

#[no_mangle]
pub extern "C" fn env_run() {
    io::init(0, "vmtest");

    log!(crate::LOG_DEF, "Setting up interrupts...");
    STATE.set(isr::State::default());
    isr::init(STATE.get_mut());
    isr::reg(isr::Vector::INSTR_PAGEFAULT.val, mmu_pf);
    isr::reg(isr::Vector::LOAD_PAGEFAULT.val, mmu_pf);
    isr::reg(isr::Vector::STORE_PAGEFAULT.val, mmu_pf);
    isr::enable_irqs();

    log!(crate::LOG_DEF, "Setting up paging...");
    paging::init();

    let virt = cfg::ENV_START;
    let pte = paging::translate(virt, PageFlags::R);
    log!(crate::LOG_DEF, "Translated virt={:#x} to PTE={:#x}", virt, pte);
    exit(0);
}
