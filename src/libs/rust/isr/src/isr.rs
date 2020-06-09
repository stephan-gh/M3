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

#[macro_use]
extern crate cfg_if;

cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        extern crate base;

        #[path = "x86_64/mod.rs"]
        mod isa;
    }
    else if #[cfg(target_arch = "arm")] {
        #[macro_use]
        extern crate base;

        #[path = "arm/mod.rs"]
        mod isa;
    }
    else {
        #[macro_use]
        extern crate base;

        #[path = "riscv/mod.rs"]
        mod isa;
    }
}

pub use self::isa::*;

type IsrFunc = extern "C" fn(state: &mut isa::State) -> *mut base::libc::c_void;

extern "C" {
    fn isr_init(stack: usize);
    fn isr_enable();
    fn isr_reg(idx: usize, func: IsrFunc);
    #[cfg(target_arch = "x86_64")]
    fn isr_set_sp(sp: usize);
}

pub fn init(stack: usize) {
    unsafe {
        isr_init(stack);
        for i in 0..isa::ISR_COUNT {
            isr_reg(i, unexpected_irq);
        }
    }
}

pub fn enable() {
    unsafe {
        isr_enable();
    }
}

pub fn reg(idx: usize, func: IsrFunc) {
    unsafe {
        isr_reg(idx, func);
    }
}

pub fn set_entry_sp(_sp: usize) {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        isr_set_sp(_sp)
    };
    #[cfg(target_arch = "riscv64")]
    write_csr!("sscratch", _sp);
}

pub extern "C" fn unexpected_irq(state: &mut State) -> *mut base::libc::c_void {
    panic!("Unexpected IRQ with {:?}", state);
}
