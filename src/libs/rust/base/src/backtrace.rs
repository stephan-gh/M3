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

use crate::arch::cfg;
use crate::arch::cpu;
use crate::math;

/// Walks up the stack and stores the return addresses into the given slice and returns the number
/// of addresses.
///
/// The function assumes that the stack is aligned by `cfg::STACK_SIZE` and ensures to not access
/// below or above the stack.
pub fn collect(addrs: &mut [usize]) -> usize {
    collect_for(cpu::base_pointer(), addrs)
}

/// Walks up the stack starting with given base pointer and stores the return addresses into the
/// given slice and returns the number of addresses.
///
/// The function assumes that the stack is aligned by `cfg::STACK_SIZE` and ensures to not access
/// below or above the stack.
pub fn collect_for(mut bp: usize, addrs: &mut [usize]) -> usize {
    if bp == 0 {
        return 0;
    }

    let base = math::round_dn(bp, cfg::STACK_SIZE);
    let end = math::round_up(bp, cfg::STACK_SIZE);
    let start = end - cfg::STACK_SIZE;

    for (i, addr) in addrs.iter_mut().enumerate() {
        if bp < start || bp >= end {
            return i;
        }

        bp = base + (bp & (cfg::STACK_SIZE - 1));
        // safety: assuming that the current BP was valid at the beginning of the function, the
        // following access is safe, because the checks above make sure that it's within our stack.
        unsafe {
            bp = cpu::backtrace_step(bp, addr);
        }
    }
    addrs.len()
}
