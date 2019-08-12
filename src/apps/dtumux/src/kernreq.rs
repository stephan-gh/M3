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
use base::cfg;
use base::dtu;
use base::envdata;
use core::intrinsics;
use core::ptr;

use arch::isr;
use arch::vma;

int_enum! {
    pub struct RCTMuxCtrl : u64 {
        const NONE    = 0;
        const STORE   = 1 << 0; // store operation required
        const RESTORE = 1 << 1; // restore operation required
        const WAITING = 1 << 2; // set by the kernel if a signal is required
        const SIGNAL  = 1 << 3; // used to signal completion to the kernel
    }
}

#[used]
#[link_section = ".rctmux"]
static RCTMUX_FLAGS: StaticCell<[u64; 2]> = StaticCell::new([0, 0]);

fn env() -> &'static mut envdata::EnvData {
    unsafe { intrinsics::transmute(cfg::ENV_START) }
}

fn flags_get() -> u64 {
    unsafe { ptr::read_volatile(&RCTMUX_FLAGS[1]) }
}

fn flags_set(val: u64) {
    unsafe { ptr::write_volatile(&mut RCTMUX_FLAGS.get_mut()[1], val) }
}

fn signal() {
    unsafe { intrinsics::atomic_fence() };
    // tell the kernel that we are ready
    flags_set(RCTMuxCtrl::SIGNAL.val);
}

pub fn handle_rctmux(state: &mut isr::State) {
    let flags = flags_get();

    if (flags & RCTMuxCtrl::RESTORE.val) != 0 {
        // notify the kernel as early as possible
        signal();

        // remember the current PE (might have changed since last switch)
        env().pe_id = flags >> 32;

        state.init(env().entry as usize, env().sp as usize);
        return;
    }

    if (flags & RCTMuxCtrl::WAITING.val) != 0 {
        signal();
    }
}

pub fn handle_ext_req(state: &mut isr::State, mut mst_req: dtu::Reg) {
    let cmd = mst_req & 0x3;
    mst_req &= !0x3;

    // ack
    dtu::DTU::set_ext_req(0);

    match From::from(cmd) {
        dtu::ExtReqOpCode::INV_PAGE => vma::flush_tlb(mst_req as usize),
        dtu::ExtReqOpCode::RCTMUX => handle_rctmux(state),
        dtu::ExtReqOpCode::STOP => state.stop(),
        _ => log!(DEF, "Unexpected cmd: {}", cmd),
    }

    #[cfg(target_arch = "arm")]
    base::dtu::DTU::clear_irq();
}
