/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2020 Nils Asmussen, Barkhausen Institut
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

//! Contains the backtrace generation function

use crate::arch::{CPUOps, CPU};
use crate::cfg;
use crate::mem::VirtAddr;
use crate::util::math;

/// Walks up the stack and stores the return addresses into the given slice and returns the number
/// of addresses.
///
/// The function assumes that the stack is aligned by `cfg::STACK_SIZE` and ensures to not access
/// below or above the stack.
pub fn collect(addrs: &mut [VirtAddr]) -> usize {
    collect_for(CPU::base_pointer(), addrs)
}

/// Walks up the stack starting with given base pointer and stores the return addresses into the
/// given slice and returns the number of addresses.
///
/// The function assumes that the stack is aligned by `cfg::STACK_SIZE` and ensures to not access
/// below or above the stack.
pub fn collect_for(mut base_ptr: VirtAddr, addrs: &mut [VirtAddr]) -> usize {
    if base_ptr.is_null() {
        return 0;
    }

    let base = math::round_dn(base_ptr, VirtAddr::from(cfg::STACK_SIZE));
    let end = math::round_up(base_ptr, VirtAddr::from(cfg::STACK_SIZE));
    let start = end - cfg::STACK_SIZE;

    for (i, addr) in addrs.iter_mut().enumerate() {
        if base_ptr < start || base_ptr >= end {
            return i;
        }

        base_ptr = base + (base_ptr & VirtAddr::from(cfg::STACK_SIZE - 1));
        // safety: assuming that the current BP was valid at the beginning of the function, the
        // following access is safe, because the checks above make sure that it's within our stack.
        unsafe {
            base_ptr = CPU::backtrace_step(base_ptr, addr);
        }
    }
    addrs.len()
}
