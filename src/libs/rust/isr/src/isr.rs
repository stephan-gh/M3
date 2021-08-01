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

use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        #[path = "x86_64/mod.rs"]
        mod isa;
    }
    else if #[cfg(target_arch = "arm")] {
        #[path = "arm/mod.rs"]
        mod isa;
    }
    else {
        #[path = "riscv/mod.rs"]
        mod isa;
    }
}

pub use self::isa::*;

use base::cell::StaticCell;
use base::pexif;
use base::tcu;

pub type IsrFunc = extern "C" fn(state: &mut State) -> *mut base::libc::c_void;

static ISRS: StaticCell<[IsrFunc; ISR_COUNT]> = StaticCell::new([unexpected_irq; ISR_COUNT]);

#[derive(Debug)]
pub enum IRQSource {
    TCU(tcu::IRQ),
    Ext(pexif::IRQId),
}

pub extern "C" fn unexpected_irq(state: &mut State) -> *mut base::libc::c_void {
    panic!("Unexpected IRQ with user state:\n{:?}", state);
}

pub fn reg(idx: usize, func: IsrFunc) {
    ISRS.get_mut()[idx] = func;
}
