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

use arch::isr;
use base::dtu;

pub fn handle_xlate(_state: &mut isr::State, _xlate_req: dtu::Reg) {
    log!(DEF, "Unexpected Xlate request");
}

pub fn handle_mmu_pf(_state: &mut isr::State) {
    log!(DEF, "Unexpected PF");
}

pub fn flush_tlb(_virt: usize) {
    log!(DEF, "Unexpected TLB flush request");
}
